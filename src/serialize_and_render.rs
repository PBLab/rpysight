//! Serialization and rendering actions

use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use arrow2::array::{Array, UInt32Array, UInt8Array};
use arrow2::datatypes::{
    DataType::{UInt32, UInt8},
    Field, Schema,
};
use arrow2::io::ipc::write::StreamWriter;
use arrow2::record_batch::RecordBatch;
use crossbeam::channel::Receiver;
use nalgebra::Point3;
use ordered_float::OrderedFloat;

use crate::point_cloud_renderer::ImageCoor;
use crate::snakes::{Coordinate, VoxelDelta};
use crate::{DISPLAY_COLORS, SUPPORTED_SPECTRAL_CHANNELS};

/// Write the data to disk in a tabular format.
///
/// This function will take the per-frame data, convert it to a clearer
/// serialization format and finally write it to disk.
pub(crate) fn serialize_data<P: AsRef<Path>>(
    recv: Receiver<FrameBuffers>,
    voxel_delta: VoxelDelta<Coordinate>,
    filename: P,
) {
    let mut coord_to_index = match CoordToIndex::try_new(&voxel_delta, filename) {
        Ok(cti) => cti,
        Err(e) => {
            error!(
                "Cannot create a file: {:?}. Not writing columnar data to disk",
                e
            );
            return;
        }
    };
    loop {
        match recv.recv() {
            Ok(new_data) => {
                let (channels, xs, ys, zs, values) = coord_to_index.map_data_to_indices(new_data);
                let rb = coord_to_index.convert_vecs_to_recordbatch(channels, xs, ys, zs, values);
                match coord_to_index.serialize_to_stream(rb) {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Failed to serialize: {:?}", e);
                    }
                };
            }
            Err(_) => break,
        };
    }
    coord_to_index.stream.finish().unwrap();
}

/// Convert the GPU-focused coordinates to array indexing.
///
/// We wish to have access to the GPU array that is rendered in each step, but
/// since that's impossible we use this struct to create a proxy - a mapping
/// between the GPU-based coordinates (probably in the range [-0.5, 0.5]) to
/// array indices ([0..len]).
struct CoordToIndex {
    row_mapping: BTreeMap<OrderedFloat<f32>, u32>,
    column_mapping: BTreeMap<OrderedFloat<f32>, u32>,
    plane_mapping: BTreeMap<OrderedFloat<f32>, u32>,
    stream: StreamWriter<File>,
    schema: Arc<Schema>,
}

impl CoordToIndex {
    /// Try to create a new mapping from the voxel delta information
    pub fn try_new<P: AsRef<Path>>(
        voxel_delta: &VoxelDelta<Coordinate>,
        filename: P,
    ) -> Result<Self> {
        let (row, col, plane) = voxel_delta.map_coord_to_index();
        info!(
            "Got the following mapping: Row: {:#?}, Col: {:#?}, Plane: {:#?}",
            row, col, plane
        );
        let schema = Schema::new(vec![
            Field::new("channel", UInt8, false),
            Field::new("x", UInt32, false),
            Field::new("y", UInt32, false),
            Field::new("z", UInt32, false),
            Field::new("value", UInt8, false),
        ]);
        let f = File::create(filename.as_ref().with_extension("arrow_stream"))?;
        info!("Writing the table to disk at: {:?}", f);
        let stream = StreamWriter::try_new(f, &schema)?;
        Ok(Self {
            row_mapping: row,
            column_mapping: col,
            plane_mapping: plane,
            stream,
            schema: Arc::new(schema),
        })
    }

    /// Convert the GPU-based coordinates and brightness levels to a table of
    /// array-focused coordinates.
    ///
    /// Note that we don't serialize the merged channel, only the individual
    /// ones.
    pub fn map_data_to_indices(
        &self,
        data: FrameBuffers,
    ) -> (Vec<u8>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u8>) {
        let length = data.len();
        let mut channels = Vec::<u8>::with_capacity(length);
        let mut xs = Vec::<u32>::with_capacity(length);
        let mut ys = Vec::<u32>::with_capacity(length);
        let mut zs = Vec::<u32>::with_capacity(length);
        let mut values = Vec::<u8>::with_capacity(length);
        for (ch, single_channel_data) in data.iter().enumerate() {
            for (point, value) in single_channel_data.iter() {
                trace!("Point to push: {:?}", point);
                let r = match self.row_mapping.get(&point.x) {
                    Some(r) => *r,
                    None => continue,
                };
                let c = match self.column_mapping.get(&point.y) {
                    Some(c) => *c,
                    None => continue,
                };
                let p = match self.plane_mapping.get(&point.z) {
                    Some(p) => *p,
                    None => continue,
                };
                // All points are not NaNs, we can add them to the buffers
                channels.push(ch as u8);
                xs.push(r);
                ys.push(c);
                zs.push(p);
                values.push(*value);
            }
        }
        (channels, xs, ys, zs, values)
    }

    /// Convert the "raw" table of data into a [`RecordBatch`] that can be
    /// streamed and serialized.
    pub fn convert_vecs_to_recordbatch(
        &self,
        channels: Vec<u8>,
        xs: Vec<u32>,
        ys: Vec<u32>,
        zs: Vec<u32>,
        values: Vec<u8>,
    ) -> RecordBatch {
        let channels = Arc::new(UInt8Array::from_trusted_len_values_iter(
            channels.into_iter(),
        ));
        let xs = Arc::new(UInt32Array::from_trusted_len_values_iter(xs.into_iter()));
        let ys = Arc::new(UInt32Array::from_trusted_len_values_iter(ys.into_iter()));
        let zs = Arc::new(UInt32Array::from_trusted_len_values_iter(zs.into_iter()));
        let values = Arc::new(UInt8Array::from_trusted_len_values_iter(values.into_iter()));
        let iter_over_vecs: Vec<Arc<dyn Array>> = vec![channels, xs, ys, zs, values];
        RecordBatch::try_new(self.schema.clone(), iter_over_vecs).unwrap()
    }

    /// Write the data to disk
    pub fn serialize_to_stream(&mut self, rb: RecordBatch) -> Result<()> {
        self.stream.write(&rb)?;
        Ok(())
    }
}

type HashMapForRendering = HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>;
type HashMapForAggregation = HashMap<Point3<OrderedFloat<f32>>, u8>;

/// A buffer for the data-to-be-rendered on a per-channel basis.
///
/// It contains two types of hashmaps - the one used for keeping rendering data
/// and the one used to keep data for aggregation and serialization.
#[derive(Clone, Debug)]
pub struct FrameBuffers {
    merge: HashMapForRendering,
    channel1: HashMapForAggregation,
    channel2: HashMapForAggregation,
    channel3: HashMapForAggregation,
    channel4: HashMapForAggregation,
    increment_color_by: f32,
}

impl<'a> FrameBuffers {
    pub fn new(increment_color_by: f32) -> Self {
        Self {
            merge: HashMap::with_capacity(600_000),
            channel1: HashMap::with_capacity(600_000),
            channel2: HashMap::with_capacity(600_000),
            channel3: HashMap::with_capacity(600_000),
            channel4: HashMap::with_capacity(600_000),
            increment_color_by,
        }
    }

    pub fn merged_channel(&mut self) -> &mut HashMapForRendering {
        &mut self.merge
    }

    pub fn clear_non_rendered_channels(&mut self) {
        self.channel1.clear();
        self.channel2.clear();
        self.channel3.clear();
        self.channel4.clear();
    }

    /// Adds the point with its color to a pixel list that will be drawn in the
    /// next rendering pass.
    /// The method is agnostic to the coordinate and the color it has,
    /// rather its job is to increment the color of the that pixel if this
    /// isn't the first time a photon has arrived at that pixel. Else it gives
    /// that pixel its default color.
    ///
    /// Each individual color channel is rendered in grayscale since they're
    /// separate, and thus they're incremented using [`GRAYSALE_STEP`]. But the
    /// merged channel shows each channel with its respective color, so this
    /// channel, marked as `frame_buffers[4]` is using a different incrementing
    /// method.
    ///
    /// Due to limitations of kiss3d all frame_buffers others than the 4th one
    /// (merge) aren't rendered, but their photons are still added to these
    /// buffers because they'll be used in the serialization process later on.
    pub fn add_to_render_queue(&mut self, point: ImageCoor, channel: usize) {
        self.add_to_merge(&point, channel);
        self.add_to_agg(&point, channel);
    }

    fn add_to_merge(&mut self, point: &ImageCoor, channel: usize) {
        let inc = self.increment_color_by;
        self.merge
            .entry(*point)
            .and_modify(|c| *c *= inc)
            .or_insert(DISPLAY_COLORS[channel]);
    }

    fn add_to_agg(&mut self, point: &ImageCoor, channel: usize) {
        self.get_agg_channel_mut(channel)
            .entry(*point)
            .and_modify(|c| *c += 1)
            .or_insert(0);
    }

    fn get_agg_channel_mut(&mut self, channel: usize) -> &mut HashMapForAggregation {
        match channel {
            0 => &mut self.channel1,
            1 => &mut self.channel2,
            2 => &mut self.channel3,
            3 => &mut self.channel4,
            _ => panic!("Wrong channel given: {}", channel),
        }
    }

    fn get_agg_channel(&self, channel: usize) -> &HashMapForAggregation {
        match channel {
            0 => &self.channel1,
            1 => &self.channel2,
            2 => &self.channel3,
            3 => &self.channel4,
            _ => panic!("Wrong channel given: {}", channel),
        }
    }

    pub fn len(&self) -> usize {
        self.merge.len()
    }

    pub(crate) fn iter(&'a self) -> FrameBuffersIter<'a> {
        self.into_iter()
    }
}

impl<'a> IntoIterator for &'a FrameBuffers {
    type Item = &'a HashMapForAggregation;
    type IntoIter = FrameBuffersIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        FrameBuffersIter {
            buf: self,
            idx: 0usize,
            len: SUPPORTED_SPECTRAL_CHANNELS,
        }
    }
}

pub struct FrameBuffersIter<'a> {
    buf: &'a FrameBuffers,
    idx: usize,
    len: usize,
}

impl<'a> Iterator for FrameBuffersIter<'a> {
    type Item = &'a HashMapForAggregation;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.len {
            self.idx += 1;
            Some(self.buf.get_agg_channel(self.idx - 1))
        } else {
            None
        }
    }
}
