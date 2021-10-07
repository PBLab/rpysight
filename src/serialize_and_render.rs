//! Serialization and rendering actions

use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use arrow2::array::{Array, Float32Array, StructArray, UInt32Array, UInt8Array};
use arrow2::datatypes::{
    DataType::{Float32, Struct, UInt32, UInt8},
    Field, Schema,
};
use arrow2::io::ipc::write::StreamWriter;
use arrow2::record_batch::RecordBatch;
use crossbeam::channel::Receiver;
use nalgebra::Point3;
use ordered_float::OrderedFloat;

use crate::snakes::{Coordinate, VoxelDelta};
use crate::SUPPORTED_SPECTRAL_CHANNELS;

/// Write the data to disk in a tabular format.
///
/// This function will take the per-frame data, convert it to a clearer
/// serialization format and finally write it to disk.
pub(crate) fn serialize_data<P: AsRef<Path>>(
    recv: Receiver<
        [HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>; SUPPORTED_SPECTRAL_CHANNELS + 1],
    >,
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
                let (channels, xs, ys, zs, colors) = coord_to_index.map_data_to_indices(new_data);
                let rb = coord_to_index.convert_vecs_to_recordbatch(channels, xs, ys, zs, colors);
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
            Field::new(
                "color",
                Struct(vec![
                    Field::new("r", Float32, false),
                    Field::new("g", Float32, false),
                    Field::new("b", Float32, false),
                ]),
                false,
            ),
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
        data: [HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>; SUPPORTED_SPECTRAL_CHANNELS + 1],
    ) -> (Vec<u8>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<Point3<f32>>) {
        let length = data[SUPPORTED_SPECTRAL_CHANNELS].len();
        let mut channels = Vec::<u8>::with_capacity(length);
        let mut xs = Vec::<u32>::with_capacity(length);
        let mut ys = Vec::<u32>::with_capacity(length);
        let mut zs = Vec::<u32>::with_capacity(length);
        let mut colors = Vec::<Point3<f32>>::with_capacity(length);
        for (ch, single_channel_data) in data[..SUPPORTED_SPECTRAL_CHANNELS].iter().enumerate() {
            for (point, color) in single_channel_data.iter() {
                debug!("Point to push: {:?}", point);
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
                colors.push(*color);
            }
        }
        (channels, xs, ys, zs, colors)
    }

    /// Convert the "raw" table of data into a [`RecordBatch`] that can be
    /// streamed and serialized.
    pub fn convert_vecs_to_recordbatch(
        &self,
        channels: Vec<u8>,
        xs: Vec<u32>,
        ys: Vec<u32>,
        zs: Vec<u32>,
        colors: Vec<Point3<f32>>,
    ) -> RecordBatch {
        let channels = Arc::new(UInt8Array::from_trusted_len_values_iter(
            channels.into_iter(),
        ));
        let xs = Arc::new(UInt32Array::from_trusted_len_values_iter(xs.into_iter()));
        let ys = Arc::new(UInt32Array::from_trusted_len_values_iter(ys.into_iter()));
        let zs = Arc::new(UInt32Array::from_trusted_len_values_iter(zs.into_iter()));
        let colors = self.convert_colors_vec_to_arrays(colors);
        let iter_over_vecs: Vec<Arc<dyn Array>> = vec![channels, xs, ys, zs, colors];
        RecordBatch::try_new(self.schema.clone(), iter_over_vecs).unwrap()
    }

    /// Create the specific structure of the colors (=brightness) to an Arrow-
    /// centered data representation.
    pub fn convert_colors_vec_to_arrays(&self, colors: Vec<Point3<f32>>) -> Arc<StructArray> {
        let length = colors.len();
        let mut colors_x = Vec::<f32>::with_capacity(length);
        let mut colors_y = Vec::<f32>::with_capacity(length);
        let mut colors_z = Vec::<f32>::with_capacity(length);
        for p in colors {
            colors_x.push(p.x);
            colors_y.push(p.y);
            colors_z.push(p.z);
        }
        let colors_x = Arc::new(Float32Array::from_trusted_len_values_iter(
            colors_x.into_iter(),
        ));
        let colors_y = Arc::new(Float32Array::from_trusted_len_values_iter(
            colors_y.into_iter(),
        ));
        let colors_z = Arc::new(Float32Array::from_trusted_len_values_iter(
            colors_z.into_iter(),
        ));
        let colors = Arc::new(StructArray::from_data(
            Struct(vec![
                Field::new("r", arrow2::datatypes::DataType::Float32, false),
                Field::new("g", arrow2::datatypes::DataType::Float32, false),
                Field::new("b", arrow2::datatypes::DataType::Float32, false),
            ]),
            vec![colors_x, colors_y, colors_z],
            None,
        ));
        colors
    }

    /// Write the data to disk
    pub fn serialize_to_stream(&mut self, rb: RecordBatch) -> Result<()> {
        self.stream.write(&rb)?;
        Ok(())
    }
}
