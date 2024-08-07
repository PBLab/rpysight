//! Methods and objects dealing with the 1D vector objects that contain the
//! temporal and spatial information of the laser's whereabouts at any given
//! moment.

extern crate log;
use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::ops::Index;

use itertools_num::linspace;
use nalgebra::DVector;
use num_traits::{FromPrimitive, ToPrimitive};
use ordered_float::{Float, OrderedFloat};
use serde::{Deserialize, Serialize};

use crate::configuration::{AppConfig, Bidirectionality, Period};
use crate::point_cloud_renderer::{ImageCoor, ProcessedEvent};

/// The image bounds as the renderer requires - start, center and end
const RENDERING_BOUNDS: (OrderedFloat<f32>, OrderedFloat<f32>, OrderedFloat<f32>) =
    (OrderedFloat(-0.5), OrderedFloat(0.0), OrderedFloat(0.5));
/// Step size between planes is affected by the total "length" or span of the rendered
/// volume.
const RENDERING_SPAN: OrderedFloat<f32> = OrderedFloat(1.0);

/// TimeTagger absolute times are i64 values that represent the number of
/// picoseconds since the start of the experiment
pub type Picosecond = i64;
/// Image coordinates are floating point values in the range [-1, 1]. We use
/// OrderedFloat to allow them to be hashed and compared.
pub type Coordinate = OrderedFloat<f32>;

/// Pixelization of the rendered volume
#[derive(Clone, Copy, Debug, PartialEq)]
struct VolumeSize {
    rows: u32,
    columns: u32,
    planes: u32,
}

impl VolumeSize {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            rows: config.rows,
            columns: config.columns,
            planes: config.planes,
        }
    }
}
/// Marker trait to allow specific types to be used as deltas between pixels -
/// for the image space rendering case the deltas are in f32, while for the
/// rendering the deltas are in Picoseconds.
pub trait ImageDelta {}

impl ImageDelta for Coordinate {}
impl ImageDelta for Picosecond {}

/// Data regarding the step size, either in image space or in picoseconds, that
/// is needed to construct the 'snake' data vector of [`TimeToCoord`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelDelta<T: ImageDelta> {
    column: T,
    row: T,
    plane: T,
    frame: T,
    volsize: VolumeSize,
    num_planes: u32,
}

impl VoxelDelta<Coordinate> {
    pub(crate) fn from_config(config: &AppConfig) -> VoxelDelta<Coordinate> {
        let jump_between_columns = RENDERING_SPAN / OrderedFloat(config.columns as f32 - 1.0);
        let jump_between_rows = RENDERING_SPAN / OrderedFloat(config.rows as f32 - 1.0);
        let jump_between_planes: OrderedFloat<f32>;
        if config.planes > 1 {
            jump_between_planes = RENDERING_SPAN / OrderedFloat(config.planes as f32 - 1.0);
            info!("The jump between planes is: {}", jump_between_planes);
        } else {
            jump_between_planes = RENDERING_SPAN;
        }

        VoxelDelta {
            column: jump_between_columns,
            row: jump_between_rows,
            plane: jump_between_planes,
            frame: OrderedFloat::nan(),
            volsize: VolumeSize::from_config(config),
            num_planes: config.planes,
        }
    }

    /// Match between a coodinate and its index in an equvilanet array.
    ///
    /// The purpose is to create a mapping between the generated coordinates,
    /// which are in imagespace - i.e. GPU world, in which the coords are
    /// confined between [-0.5, 0.5], and the outside world, where array
    /// indexing uses the actual index of the array. This is helpful when we
    /// wish to render the data elsewhere - giving it a coordinate in the range
    /// [-0.5, 0.5] isn't as helpful as giving it a number between 0 and the
    /// size of the array.
    pub(crate) fn map_coord_to_index(
        &self,
    ) -> (
        BTreeMap<OrderedFloat<f32>, u32>,
        BTreeMap<OrderedFloat<f32>, u32>,
    ) {
        let coord_to_index_rows =
            VoxelDelta::create_single_coord_idx_mapping(self.volsize.rows, self.row);
        let coord_to_index_cols =
            VoxelDelta::create_single_coord_idx_mapping(self.volsize.columns, self.column);
        (coord_to_index_rows, coord_to_index_cols)
    }

    /// Create a single mapping between a coordinate vector and its index
    fn create_single_coord_idx_mapping(
        num: u32,
        step: OrderedFloat<f32>,
    ) -> BTreeMap<OrderedFloat<f32>, u32> {
        let mut tree = BTreeMap::<OrderedFloat<f32>, u32>::new();
        (0..num).for_each(|idx| {
            tree.insert(
                RENDERING_BOUNDS.0 + (OrderedFloat::<f32>(idx as f32) * step),
                idx,
            );
        });
        tree
    }
}

impl VoxelDelta<Picosecond> {
    pub(crate) fn from_config(config: &AppConfig) -> VoxelDelta<Picosecond> {
        let time_between_columns = VoxelDelta::calc_time_between_columns(&config);
        let time_between_rows = VoxelDelta::calc_time_between_rows(&config);
        let time_between_planes = VoxelDelta::calc_time_between_planes(&config);
        let time_between_frames = config.frame_dead_time;
        VoxelDelta {
            column: time_between_columns,
            row: time_between_rows,
            plane: time_between_planes,
            frame: time_between_frames,
            volsize: VolumeSize::from_config(config),
            num_planes: config.planes,
        }
    }

    /// Number of picosecond between consecutive voxels in a single 2D line,
    /// barring any TAG-related scanning
    fn calc_time_between_columns(config: &AppConfig) -> Picosecond {
        let effective_line_period = VoxelDelta::calc_effective_line_period(&config);
        effective_line_period / (config.columns as Picosecond)
    }

    /// The time the scanner is effectively inside the image space. This time
    /// is different than the scan period due to the fill fraction
    fn calc_effective_line_period(config: &AppConfig) -> Picosecond {
        ((*config.scan_period / 2) as f64 * (config.fill_fraction / 100.0) as f64).round()
            as Picosecond
    }

    /// Number of Picoseconds between consecutive Z-planes
    fn calc_time_between_planes(config: &AppConfig) -> Picosecond {
        (*config.tag_period / 2) / (config.planes as Picosecond)
    }

    /// Returns the number of picoseconds since we last were on a pixel.
    ///
    /// If the scan is bidirectional, this time corresponds to the time it
    /// the mirror to slow down, turn and accelerate back to the next line in
    /// the opposite direction. If the scan is unidirectional then the method
    /// factors in the time it takes the mirror to move to its starting
    /// position in the opposite side of the image.
    fn calc_time_between_rows(config: &AppConfig) -> Picosecond {
        let full_time_per_line = *config.scan_period / 2;
        let effective_line_period = VoxelDelta::calc_effective_line_period(&config);
        let deadtime_during_rotation = full_time_per_line - effective_line_period;
        match config.bidir {
            Bidirectionality::Bidir => deadtime_during_rotation,
            Bidirectionality::Unidir => {
                (full_time_per_line as i64) + (2 * deadtime_during_rotation)
            }
        }
    }
}

/// The mapping\pairing between a time in ps since the start of the experiment
/// and the image-space coordinate that this time corresponds to for the
/// current rendered volume.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeCoordPair {
    pub end_time: Picosecond,
    pub coord: ImageCoor,
}

impl TimeCoordPair {
    pub fn new(end_time: Picosecond, coord: ImageCoor) -> TimeCoordPair {
        TimeCoordPair { end_time, coord }
    }
}

/// Connect each timestamp to its coordinate.
///
/// This struct matches between the Picosecond-based partitioning of the planes
/// and the coordinate-based one, by allowing its users to index with a
/// Picosecond value and get the matching coordinate in imagespace back.
///
/// Matching is done via linear search currently, although using some B-TreeMap
/// could potentially be faster.
#[derive(Clone, Debug)]
struct IntervalToCoordMap {
    im_vec: DVector<Coordinate>,
    time_vec: DVector<Picosecond>,
}

impl IntervalToCoordMap {
    /// Creata a new map with the given two sets of data
    pub fn new(im_vec: DVector<Coordinate>, time_vec: DVector<Picosecond>) -> Self {
        assert_eq!(im_vec.len(), time_vec.len());
        Self { im_vec, time_vec }
    }

    /// Create an empty map, possibly for naive instatiation
    pub fn empty() -> Self {
        Self {
            im_vec: DVector::from_vec(vec![OrderedFloat(0.0f32)]),
            time_vec: DVector::from_vec(vec![0i64]),
        }
    }

    pub fn get_im_vec(&self) -> DVector<Coordinate> {
        self.im_vec.clone()
    }
}

impl Index<Picosecond> for IntervalToCoordMap {
    type Output = Coordinate;

    fn index(&self, time: Picosecond) -> &Self::Output {
        let idx = self.time_vec.iter().position(|x| time <= *x);
        match idx {
            Some(loc) => &self.im_vec[loc],
            None => &OrderedFloat(0.0f32),
        }
    }
}

/// Behavior related to the 1D snake which contains the allocated photon data.
///
/// The snake may be a 2D- or 3D-based snake, and thus it's generic over the
/// number of dimensions N
pub trait Snake {
    /// Returns the value assigned to the snake's capacity
    ///
    /// For 2D imaging it's num_rows * (num_columns + 1), and for 3D we add
    /// in the number of planes.
    ///
    /// These numbers take into account a cell before each frame which captures
    /// photons arriving between frames, and a cell we remove from the last row
    /// which is not needed and a cell that is added so that we don't over-
    /// allocate.
    fn calc_snake_length(&self, config: &AppConfig) -> usize;

    /// Create an empty snake to be later populated by the 'generate' methods
    fn allocate_snake(&self, config: &AppConfig) -> Vec<TimeCoordPair> {
        let capacity = self.calc_snake_length(config);
        Vec::<TimeCoordPair>::with_capacity(capacity)
    }

    fn get_earliest_frame_time(&self) -> Picosecond;

    /// Initialize the time -> coordinate mapping assuming that we're starting
    /// the imaging at time `offset` of the experiment.
    ///
    /// The function matches on the scanning directionality of the experiment
    /// and calls the proper methods accordingly.
    ///
    /// Once the mapping vector is initialized, subsequent frames only have to
    /// update the "end_time" field in each cell according to the current frame
    /// offset.
    fn from_acq_params(config: &AppConfig, offset: Picosecond) -> Self
    where
        Self: Sized;

    /// Generate the per-row snake vectors for the Picosecond part.
    ///
    /// Each row of the final snake is similar to its predecessor, with the
    /// values of the end time fields incremented by this row's offset. This
    /// method generates this general vector - once for the ps data and one for
    /// the pixel data - and sends it to be copied multiple times with slight
    /// changes later on.
    fn construct_row_ps_snake(
        &self,
        num_columns: usize,
        voxel_delta_ps: &VoxelDelta<Picosecond>,
    ) -> DVector<Picosecond> {
        // We add to the naive capacity 1 due to the cell containing all events
        // arriving in between frames. The number of columns for the capacity
        // calculation includes a fake column containing the photons arriving
        // during mirror rotation. Their coordinate will contain a NaN value,
        // which means that it will not be rendered.
        let column_deltas_ps = DVector::<Picosecond>::from_fn(num_columns, |i, _| {
            (i as Picosecond) * voxel_delta_ps.column + voxel_delta_ps.column
        });
        // Manually add the cell corresponding to events arriving during mirror
        // rotation
        let end_of_rotation_value = column_deltas_ps[num_columns - 1] + voxel_delta_ps.row;
        let column_deltas_ps = column_deltas_ps.insert_rows(num_columns, 1, end_of_rotation_value);
        column_deltas_ps
    }

    /// Generate the per-row snake vectors for the imagespace part.
    ///
    /// Each row of the final snake is similar to its predecessor, with the
    /// values of the end time fields incremented by this row's offset. This
    /// method generates this general vector - once for the ps data and one for
    /// the pixel data - and sends it to be copied multiple times with slight
    /// changes later on.
    fn construct_row_im_snake(
        &self,
        num_columns: usize,
        voxel_delta_im: &VoxelDelta<Coordinate>,
    ) -> DVector<Coordinate> {
        let column_deltas_imagespace = DVector::<Coordinate>::from_fn(num_columns, |i, _| {
            OrderedFloat::<f32>::from_usize(i).unwrap() * voxel_delta_im.column
        });
        // The events during mirror rotation will be discarded - The NaN takes
        // care of that
        let column_deltas_imagespace = column_deltas_imagespace
            .add_scalar(RENDERING_BOUNDS.0)
            .insert_rows(num_columns, 1, OrderedFloat(f32::NAN));
        column_deltas_imagespace
    }

    /// Generate an imagespace row snake for the bidirectional rows.
    ///
    /// The odd rows should have the order of the cells in their snakes
    /// reversed.
    fn reverse_row_imagespace(
        &self,
        column_deltas_imagespace: &DVector<Coordinate>,
    ) -> DVector<Coordinate> {
        let mut column_deltas_imagespace_rev: Vec<Coordinate> = (column_deltas_imagespace
            .iter()
            .rev()
            .copied()
            .collect::<Vec<Coordinate>>())
        .clone();
        let nan = column_deltas_imagespace_rev.remove(0);
        column_deltas_imagespace_rev.push(nan);
        DVector::from_vec(column_deltas_imagespace_rev)
    }

    /// Generate a row Picosecond snake for the bidirectional rows.
    ///
    /// The odd rows should have the order of the cells in their snakes
    /// reversed.
    fn reverse_row_picosecond(
        &self,
        column_deltas_ps: &DVector<Picosecond>,
        line_shift: Picosecond,
    ) -> DVector<Picosecond> {
        column_deltas_ps.add_scalar(line_shift)
    }

    /// Update the existing data to accommodate the new frame.
    ///
    /// This function is triggered from the 'tag_to_coord' method once an event
    /// with a time tag later than the last possible voxel is detected. It
    /// currently updates the exisitng data based on a guesstimation regarding
    /// data quality, i.e. we don't do any error checking what-so-ever, we
    /// simply trust in the data being not faulty.
    fn update_snake_for_next_frame(&mut self, next_frame_at: Picosecond);

    /// Handle a time tag by finding its corresponding coordinate in image
    /// space using linear search.
    ///
    /// The arriving time tag should have a coordinate associated with it. To
    /// find it we traverse the boundary vector (snake) until we find the
    /// coordinate that has an end time longer than the specified time. If the
    /// tag has a time longer than the end of the current frame this function
    /// is also in charge of calling the 'update' method to generate a new
    /// snake for the next frame.
    ///
    /// This implementation is based on linear search because it's assumed that
    /// during peak event rates most pixels (snake cells) will be populated by
    /// at least one event, which means that this search will be stopped after
    /// a single step, or perhaps two. This should, in theory, be faster than
    /// other options for this algorithm (which are currently unexplored), such
    /// as binary search, hashmap or an interval tree.
    fn time_to_coord_linear(&mut self, time: Picosecond, ch: usize) -> ProcessedEvent;

    /// Return the Z coordinate of a timetag.
    ///
    /// In the 2D case this method should be left unimplemented.
    fn update_z_coord(&self, _coord: ImageCoor, _time: Picosecond) -> ImageCoor {
        ImageCoor::new(RENDERING_BOUNDS.1, RENDERING_BOUNDS.1, RENDERING_BOUNDS.1)
    }

    /// Returns the planes imagespace vector, or None if it doesn't exist
    /// (2D case)
    fn get_z_imagespace_planes(&self) -> Option<DVector<Coordinate>>;

    /// Handles a new TAG lens start-of-cycle event
    fn new_taglens_period(&mut self, _time: Picosecond) -> ProcessedEvent {
        ProcessedEvent::NoOp
    }

    fn new_laser_event(&self, _time: Picosecond) -> ProcessedEvent {
        ProcessedEvent::NoOp
    }

    fn dump(&self, _time: Picosecond) -> ProcessedEvent {
        ProcessedEvent::NoOp
    }

    fn get_voxel_delta_im(&self) -> VoxelDelta<Coordinate>;
}

/// Data and logic for finding the image-space coordinates for the given
/// time tags.
///
/// The goal of this data structure is to hold the necessary information to
/// efficiently assign to each arriving timetag its coordinate in image space,
/// i.e. in the range of [0, 1.0] in each dimension. This information includes
/// data arriving from the user as well as computations performed locally that
/// are also needed to keep the 'context' of the app between rendered frames.
///
/// During this object's initialization step, a vector of time -> coordinate
/// is built and populated with the predicted timings based on the current
/// experimental configuration. This vector, referred to here as a snake,
/// represents a 1D version of the image. Its length is the total number of
/// pixels that the scanning laser will point to during each frame. For 2D
/// images this length is the total pixel count of the image, but when using a
/// TAG lens some pixels might never receive a visit in a single frame due to
/// the resonant nature of that scanning element.
///
/// The snake's individual cells are small structs that map the time in ps that
/// corresponds to this pixel with its coordinate. For example, assuming 256
/// columns in the resulting image and a scanning frequency of about 8 kHz, it
/// will use a pixel dwell time of about 125,000 ps as the 'bins' between
/// consecutive pixels or voxels in the image. In this case, the value of the
/// first cell in the snake will be 125,000 and 1/256, while the next cell will
/// contain 250,000 and 2/256. This initialization step also takes into account
/// the dead time between consecutive rows and the fill fraction of the
/// experimental setup.
///
/// Once initialized, the logic is straight forward - look for the first cell
/// in the snake that has a ps value smaller than the currently-arriving time
/// tag. Once found, return the image-space coordinate of that cell so that the
/// photon could be placed in that pixel. By pre-populating this snake with the
/// suitable time -> coordinate conversion we should save some lookup time.
#[derive(Debug)]
pub struct TwoDimensionalSnake {
    /// A vector of end times with their corresponding image-space
    /// coordinates.
    data: Vec<TimeCoordPair>,
    /// The index in data that was last used to retrieve a
    /// coordinates. We keep it to look for the next matching end time only from
    /// that value onward.
    last_accessed_idx: usize,
    /// The end time for the frame. Useful to quickly check
    /// whether a time tag belongs in the next frame.
    max_frame_time: Picosecond,
    /// Deltas in ps of consecutive pixels, lines, etc.
    voxel_delta_ps: VoxelDelta<Picosecond>,
    /// Deltas in image space of consecutive pixels, lines, etc.
    voxel_delta_im: VoxelDelta<Coordinate>,
    /// The earliest time of the first voxel
    earliest_frame_time: Picosecond,
    /// Time between the end of one frame and the start of the next
    frame_dead_time: Picosecond,
}

pub struct ThreeDimensionalSnake {
    /// A vector of end times with their corresponding image-space
    /// coordinates.
    data: Vec<TimeCoordPair>,
    /// The index in data that was last used to retrieve a
    /// coordinates. We keep it to look for the next matching end time only from
    /// that value onward.
    last_accessed_idx: usize,
    /// Last signal received from the TAG Lens
    last_taglens_time: Picosecond,
    /// A mapping between the arrival time of an event relative to the TAG lens
    /// period and the assigned coordinate.
    tag_deltas_to_coord: IntervalToCoordMap,
    /// The end time for the frame. Useful to quickly check
    /// whether a time tag belongs in the next frame.
    max_frame_time: Picosecond,
    /// Deltas in ps of consecutive pixels, lines, etc.
    voxel_delta_ps: VoxelDelta<Picosecond>,
    /// Deltas in image space of consecutive pixels, lines, etc.
    voxel_delta_im: VoxelDelta<Coordinate>,
    /// The earliest time of the first voxel
    earliest_frame_time: Picosecond,
    /// Time between the end of one frame and the start of the next
    frame_dead_time: Picosecond,
}

impl TwoDimensionalSnake {
    /// Initialize this struct with naive default parameters.
    ///
    /// Since initializing this struct is a complex task this function provides
    /// a simple implementation just so we could have the struct at our
    /// disposal. It helps, for example, that once it's initialized we can use
    /// methods from the Snake trait to refine the values of the field of this
    /// struct.
    ///
    /// This method is intentionally kept private since the proper way to
    /// initialize this object is using the "from_acq_params" function.
    pub fn naive_init(config: &AppConfig) -> Self {
        let voxel_delta_ps = VoxelDelta::<Picosecond>::from_config(&config);
        let voxel_delta_im = VoxelDelta::<Coordinate>::from_config(&config);

        Self {
            data: Vec::new(),
            voxel_delta_ps,
            voxel_delta_im,
            last_accessed_idx: 0,
            max_frame_time: 0,
            earliest_frame_time: 0,
            frame_dead_time: 0,
        }
    }

    /// Constructs the 1D vector mapping the time of arrival to image-space
    /// coordinates.
    ///
    /// This vector is essentially identical to a flattened version of all
    /// pixels of the image, with two main differences: The first, it takes
    /// into account the bidirectionality of the scanner, i.e. odd rows are
    /// 'concatenated' in reverse and are given a phase shift. The second, per
    /// frame it has an extra "row" and an extra column that should contain
    /// photons arriving between frames and while the scanner was rotating,
    /// respectively.
    ///
    /// What this function does is traverse all cells of the vector and
    /// populate them with the mapping ps -> coordinate. It's also aware of the
    /// side column in each row which is 'garbage' and populated with a NaN
    /// value here to not be rendered.
    fn update_naive_with_parameters_bidir(
        mut self,
        config: &AppConfig,
        column_deltas_ps: &mut DVector<Picosecond>,
        column_deltas_imagespace: &DVector<Coordinate>,
        offset: Picosecond,
    ) -> TwoDimensionalSnake {
        // Add the cell capturing all photons arriving between frames
        let deadtime_during_rotation = column_deltas_ps[column_deltas_ps.len() - 1];
        let mut line_offset: Picosecond = offset;
        let column_deltas_imagespace_rev = self.reverse_row_imagespace(column_deltas_imagespace);
        let column_deltas_ps_bidir =
            self.reverse_row_picosecond(column_deltas_ps, config.line_shift);
        let mut row_coord: Coordinate;
        for row in (0..config.rows).step_by(2) {
            // Start with the unidir row
            row_coord = (OrderedFloat(row as f32) * self.voxel_delta_im.row) + RENDERING_BOUNDS.0;
            TwoDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace,
                &column_deltas_ps,
                row_coord,
                line_offset,
            );
            line_offset += deadtime_during_rotation;
            // Now the bidir row
            row_coord =
                (OrderedFloat((row + 1) as f32) * self.voxel_delta_im.row) + RENDERING_BOUNDS.0;
            TwoDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace_rev,
                &column_deltas_ps_bidir,
                row_coord,
                line_offset,
            );
            line_offset += deadtime_during_rotation;
        }
        let _ = self.data.pop(); // Last element is the mirror rotation for the
                                 // last row, which is unneeded.
        let max_frame_time = self.data[self.data.len() - 1].end_time;
        info!("2D bidir Snake built");
        TwoDimensionalSnake {
            data: self.data,
            last_accessed_idx: 0,
            max_frame_time,
            voxel_delta_ps: self.voxel_delta_ps,
            voxel_delta_im: self.voxel_delta_im,
            earliest_frame_time: offset,
            frame_dead_time: config.frame_dead_time,
        }
    }

    /// Update the time -> coordinate snake when we're scanning.
    ///
    /// This method is also used in the bidirectional case, except it's used
    /// once for the even ones and again for the odd ones with different
    /// parameters.
    fn push_pair_unidir(
        snake: &mut Vec<TimeCoordPair>,
        column_deltas_imagespace: &DVector<Coordinate>,
        column_deltas_ps: &DVector<Picosecond>,
        row_coord: Coordinate,
        line_offset: Picosecond,
    ) {
        for (column_delta_im, column_delta_ps) in column_deltas_imagespace
            .into_iter()
            .zip(column_deltas_ps.into_iter())
        {
            let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, OrderedFloat(0.0));
            snake.push(TimeCoordPair::new(
                column_delta_ps + line_offset,
                cur_imcoor,
            ));
        }
    }

    fn update_naive_with_parameters_unidir(
        mut self,
        config: &AppConfig,
        column_deltas_ps: &mut DVector<Picosecond>,
        column_deltas_imagespace: &DVector<Coordinate>,
        offset: Picosecond,
    ) -> TwoDimensionalSnake {
        // Add the cell capturing all photons arriving between frames
        let line_len = column_deltas_ps.len();
        let offset_per_row = column_deltas_ps[line_len - 1];
        let mut line_offset: Picosecond = offset;
        for row in 0..config.rows {
            let row_coord =
                (OrderedFloat(row as f32) * self.voxel_delta_im.row) + RENDERING_BOUNDS.0;
            TwoDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace,
                &column_deltas_ps,
                row_coord,
                line_offset,
            );
            line_offset += offset_per_row;
        }
        let _ = self.data.pop();
        let max_frame_time = self.data[self.data.len() - 1].end_time;
        info!("2D unidir snake finished");
        TwoDimensionalSnake {
            data: self.data,
            last_accessed_idx: 0,
            max_frame_time,
            voxel_delta_ps: self.voxel_delta_ps,
            voxel_delta_im: self.voxel_delta_im,
            earliest_frame_time: offset,
            frame_dead_time: config.frame_dead_time,
        }
    }
}

impl ThreeDimensionalSnake {
    /// Initialize this struct with naive default parameters.
    ///
    /// Since initializing this struct is a complex task this function provides
    /// a simple implementation just so we could have the struct at our
    /// disposal. It helps, for example, that once it's initialized we can use
    /// methods from the Snake trait to refine the values of the field of this
    /// struct.
    ///
    /// This method is intentionally kept private since the proper way to
    /// initialize this object is using the "from_acq_params" function.
    fn naive_init(config: &AppConfig) -> Self {
        let voxel_delta_ps = VoxelDelta::<Picosecond>::from_config(&config);
        let voxel_delta_im = VoxelDelta::<Coordinate>::from_config(&config);

        Self {
            data: Vec::new(),
            voxel_delta_ps,
            voxel_delta_im,
            last_accessed_idx: 0,
            last_taglens_time: 0,
            tag_deltas_to_coord: IntervalToCoordMap::empty(),
            max_frame_time: 0,
            earliest_frame_time: 0,
            frame_dead_time: 0,
        }
    }

    fn push_pair_unidir(
        snake: &mut Vec<TimeCoordPair>,
        column_deltas_imagespace: &DVector<Coordinate>,
        column_deltas_ps: &DVector<Picosecond>,
        row_coord: Coordinate,
        line_offset: Picosecond,
    ) {
        for (column_delta_im, column_delta_ps) in column_deltas_imagespace
            .into_iter()
            .zip(column_deltas_ps.into_iter())
        {
            let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, OrderedFloat(0.0));
            snake.push(TimeCoordPair::new(
                column_delta_ps + line_offset,
                cur_imcoor,
            ));
        }
    }

    /// Create a Z-planes coordinate vector.
    ///
    /// This method assigns the coordinates to each plane of the volume by
    /// dividing the Z axis into three parts, in accordance with a sine curve:
    /// The rising part (up to pi/2), the decending part (pi/2, 3pi/2) and the
    // last rise (3pi/2, 2pi).
    fn create_planes_snake_imagespace(&self, planes: usize) -> DVector<Coordinate> {
        let half_planes = planes / 2 - 1;
        let phase_limits_1_to_m1 = DVector::<Coordinate>::from_iterator(
            planes,
            linspace::<Coordinate>(RENDERING_BOUNDS.2, RENDERING_BOUNDS.0, planes),
        );
        let phase_limits_0_to_1 = &mut phase_limits_1_to_m1.rows(1, half_planes).clone_owned();
        phase_limits_0_to_1.as_mut_slice().reverse();
        let phase_limits_0_to_1 = DVector::<Coordinate>::from_iterator(
            half_planes,
            phase_limits_0_to_1.into_iter().map(|x| *x),
        );
        let phase_limits_m1_to_0 = &mut phase_limits_1_to_m1
            .rows(half_planes + 1, half_planes)
            .clone_owned();
        phase_limits_m1_to_0.as_mut_slice().reverse();
        let phase_limits_m1_to_0 = DVector::<Coordinate>::from_iterator(
            half_planes,
            phase_limits_m1_to_0.into_iter().map(|x| *x),
        );

        let mut all_phases =
            DVector::<Coordinate>::repeat(half_planes + half_planes + planes, OrderedFloat(0.0f32));
        all_phases
            .rows_mut(0, phase_limits_0_to_1.len())
            .set_column(0, &phase_limits_0_to_1);
        all_phases
            .rows_mut(phase_limits_0_to_1.len(), phase_limits_1_to_m1.len())
            .set_column(0, &phase_limits_1_to_m1);
        all_phases
            .rows_mut(
                phase_limits_0_to_1.len() + phase_limits_1_to_m1.len(),
                phase_limits_m1_to_0.len(),
            )
            .set_column(0, &phase_limits_m1_to_0);
        info!("The phases vector we made is: {:#?}", all_phases);
        all_phases
    }

    /// Create a Z-planes Picosecond vector.
    ///
    /// This method assigns the Picosecond value to each plane of the volume by
    /// dividing the Z axis into three parts, in accordance with a sine curve:
    /// The rising part (up to pi/2), the decending part (pi/2, 3pi/2) and the
    /// last rise (3pi/2, 2pi).
    ///
    /// The Planes imagespace vector is multiplied by 2 before the computation
    /// because the way this arcsine function works is with the planes
    /// given between [-1.0, 1.0] rather than [-0.5, 0.5].
    fn create_planes_snake_ps(
        &self,
        planes: &DVector<Coordinate>,
        period: Picosecond,
    ) -> DVector<Picosecond> {
        let quarter_period = OrderedFloat::from_i64(period / 4).unwrap();
        let num_planes = planes.len();
        let half = num_planes / 2 + 1;
        let firstq = half / 2 - 1;
        let lastq = firstq + half;
        let mut asin = planes
            .map(|x| x * OrderedFloat(2.0))
            .map(|x| x.asin() / (PI / 2.0));
        let mut sine_ps = DVector::<Coordinate>::repeat(num_planes, quarter_period);
        // First quarter of phase
        sine_ps
            .rows_mut(0, firstq)
            .component_mul_assign(&asin.rows(0, firstq));
        // Middle two quarters
        sine_ps
            .rows_mut(firstq, half)
            .component_mul_assign(&asin.rows_mut(firstq, half).map(|x| OrderedFloat(1.0) - x));
        sine_ps
            .rows_mut(firstq, half)
            .add_scalar_mut(quarter_period);
        // Last quarter
        sine_ps
            .rows_mut(lastq, firstq)
            .component_mul_assign(&asin.rows_mut(lastq, firstq).map(|x| OrderedFloat(1.0) + x));
        sine_ps
            .rows_mut(lastq, firstq)
            .add_scalar_mut(OrderedFloat(3.0) * quarter_period);

        let sine_ps = sine_ps.map(|x| x.to_i64().unwrap());
        info!("The PS snake we made: {:#?}", sine_ps);
        sine_ps
    }

    /// Constructs the 1D vector mapping the time of arrival to image-space
    /// coordinates.
    ///
    /// This vector is essentially identical to a flattened version of all
    /// pixels of the image, with two main differences: The first, it takes
    /// into account the bidirectionality of the scanner, i.e. odd rows are
    /// 'concatenated' in reverse and are given a phase shift. The second, per
    /// frame it has an extra "row" and an extra column that should contain
    /// photons arriving between frames and while the scanner was rotating,
    /// respectively.
    ///
    /// What this function does is traverse all cells of the vector and
    /// populate them with the mapping ps -> coordinate. It's also aware of the
    /// side column in each row which is 'garbage' and populated with a NaN
    /// value here to not be rendered.
    fn update_naive_with_parameters_bidir(
        mut self,
        config: &AppConfig,
        column_deltas_ps: &mut DVector<Picosecond>,
        column_deltas_imagespace: &DVector<Coordinate>,
        offset: Picosecond,
    ) -> Self {
        // Add the cell capturing all photons arriving between frames
        let deadtime_during_rotation = column_deltas_ps[column_deltas_ps.len() - 1];
        let mut line_offset: Picosecond = offset;
        let column_deltas_imagespace_rev = self.reverse_row_imagespace(column_deltas_imagespace);
        let column_deltas_ps_bidir =
            self.reverse_row_picosecond(column_deltas_ps, config.line_shift);
        let mut row_coord: Coordinate;
        for row in (0..config.rows).step_by(2) {
            // Start with the unidir row
            row_coord = (OrderedFloat(row as f32) * self.voxel_delta_im.row) + RENDERING_BOUNDS.0;
            ThreeDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace,
                &column_deltas_ps,
                row_coord,
                line_offset,
            );
            line_offset += deadtime_during_rotation;
            // Now the bidir row
            row_coord =
                (OrderedFloat((row + 1) as f32) * self.voxel_delta_im.row) + RENDERING_BOUNDS.0;
            ThreeDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace_rev,
                &column_deltas_ps_bidir,
                row_coord,
                line_offset,
            );
            line_offset += deadtime_during_rotation;
        }
        let _ = self.data.pop(); // Last element is the mirror rotation for the
                                 // last row, which is unneeded.
        let max_frame_time = self.data[self.data.len() - 1].end_time;
        let tag_deltas_to_coord =
            self.build_taglens_delta_to_coord_mapping(config.planes, config.tag_period);
        info!("3D bidir Snake built");
        ThreeDimensionalSnake {
            data: self.data,
            last_accessed_idx: 0,
            last_taglens_time: 0,
            tag_deltas_to_coord,
            max_frame_time,
            voxel_delta_ps: self.voxel_delta_ps,
            voxel_delta_im: self.voxel_delta_im,
            earliest_frame_time: offset,
            frame_dead_time: config.frame_dead_time,
        }
    }

    fn update_naive_with_parameters_unidir(
        mut self,
        config: &AppConfig,
        column_deltas_ps: &mut DVector<Picosecond>,
        column_deltas_imagespace: &DVector<Coordinate>,
        offset: Picosecond,
    ) -> ThreeDimensionalSnake {
        // Add the cell capturing all photons arriving between frames
        let line_len = column_deltas_ps.len();
        let offset_per_row = column_deltas_ps[line_len - 1];
        let mut line_offset: Picosecond = offset;
        for row in 0..config.rows {
            let row_coord =
                (OrderedFloat(row as f32) * self.voxel_delta_im.row) + RENDERING_BOUNDS.0;
            ThreeDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace,
                &column_deltas_ps,
                row_coord,
                line_offset,
            );
            line_offset += offset_per_row;
        }
        let _ = self.data.pop();
        let max_frame_time = self.data[self.data.len() - 1].end_time;
        let tag_deltas_to_coord =
            self.build_taglens_delta_to_coord_mapping(config.planes, config.tag_period);
        info!("3D unidir snake finished");
        ThreeDimensionalSnake {
            data: self.data,
            last_accessed_idx: 0,
            max_frame_time,
            voxel_delta_ps: self.voxel_delta_ps,
            voxel_delta_im: self.voxel_delta_im,
            earliest_frame_time: offset,
            frame_dead_time: config.frame_dead_time,
            last_taglens_time: 0,
            tag_deltas_to_coord,
        }
    }

    /// Construct a mapping between the arrival time of an event and its
    /// coordinate in the planes dimension for 3D imaging.
    fn build_taglens_delta_to_coord_mapping(
        &self,
        planes: u32,
        period: Period,
    ) -> IntervalToCoordMap {
        let snake_im = self.create_planes_snake_imagespace(planes as usize);
        let snake_ps = self.create_planes_snake_ps(&snake_im, *period);
        IntervalToCoordMap::new(snake_im, snake_ps)
    }
}

impl Snake for TwoDimensionalSnake {
    fn from_acq_params(config: &AppConfig, offset: Picosecond) -> TwoDimensionalSnake {
        let mut twod_snake = TwoDimensionalSnake::naive_init(config);
        twod_snake.data = twod_snake.allocate_snake(&config);
        twod_snake.data.push(TimeCoordPair::new(
            offset,
            ImageCoor::new(
                OrderedFloat(f32::NAN),
                OrderedFloat(f32::NAN),
                OrderedFloat(f32::NAN),
            ),
        ));
        let num_columns = config.columns as usize;
        let mut column_deltas_ps =
            twod_snake.construct_row_ps_snake(num_columns, &twod_snake.voxel_delta_ps);
        let column_deltas_imagespace =
            twod_snake.construct_row_im_snake(num_columns, &twod_snake.voxel_delta_im);
        match config.bidir {
            Bidirectionality::Bidir => twod_snake.update_naive_with_parameters_bidir(
                &config,
                &mut column_deltas_ps,
                &column_deltas_imagespace,
                offset,
            ),
            Bidirectionality::Unidir => twod_snake.update_naive_with_parameters_unidir(
                &config,
                &mut column_deltas_ps,
                &column_deltas_imagespace,
                offset,
            ),
        }
    }

    fn get_voxel_delta_im(&self) -> VoxelDelta<Coordinate> {
        self.voxel_delta_im.clone()
    }

    fn get_earliest_frame_time(&self) -> Picosecond {
        self.earliest_frame_time
    }

    /// Returns the value assigned to the snake's capacity
    ///
    /// For 2D imaging it's num_rows * (num_columns + 1)
    ///
    /// These numbers take into account a cell before each frame which captures
    /// photons arriving between frames, and a cell we remove from the last row
    /// which is not needed and a cell that is added so that we don't over-
    /// allocate..
    fn calc_snake_length(&self, config: &AppConfig) -> usize {
        let baseline_count = ((config.columns + 1) * config.rows) as usize;
        baseline_count + 1
    }

    /// Handle a time tag by finding its corresponding coordinate in image
    /// space using linear search.
    ///
    /// The arriving time tag should have a coordinate associated with it. To
    /// find it we traverse the boundary vector (snake) until we find the
    /// coordinate that has an end time longer than the specified time. If the
    /// tag has a time longer than the end of the current frame this function
    /// is also in charge of calling the 'update' method to generate a new
    /// snake for the next frame.
    ///
    /// This implementation is based on linear search because it's assumed that
    /// during peak event rates most pixels (snake cells) will be populated by
    /// at least one event, which means that this search will be stopped after
    /// a single step, or perhaps two. This should, in theory, be faster than
    /// other options for this algorithm (which are currently unexplored), such
    /// as binary search, hashmap or an interval tree.
    fn time_to_coord_linear(&mut self, time: i64, ch: usize) -> ProcessedEvent {
        if time > self.max_frame_time {
            debug!(
                "Photon arrived after end of Frame! Our time: {}, Max time: {}",
                time, self.max_frame_time
            );
            self.update_snake_for_next_frame(self.max_frame_time + self.frame_dead_time);
            return ProcessedEvent::PhotonNewFrame;
        }
        let mut additional_steps_taken = 0usize;
        let mut coord = None;
        for pair in &self.data[self.last_accessed_idx..] {
            if time <= pair.end_time {
                trace!(
                    "Found a point on the snake! Pair: {:?}; Time: {}; Additional steps taken: {}; Channel: {}",
                    pair, time, additional_steps_taken, ch
                );
                self.last_accessed_idx += additional_steps_taken;
                coord = Some(pair.coord);
                break;
            }
            additional_steps_taken += 1;
        }
        // Makes sure that we indeed captured some cell. This can be avoided in
        // principle but I'm still not confident enough in this implementation.
        if let Some(coord) = coord {
            ProcessedEvent::Displayed(coord, ch)
        } else {
            error!(
                "Coordinate remained unpopulated. self.data: {:?}\nAdditional steps taken: {}",
                &self.data[self.last_accessed_idx..],
                additional_steps_taken
            );
            panic!("Coordinate remained unpopulated for some reason. Investigate!")
        }
    }

    /// Update the existing data to accommodate the new frame.
    ///
    /// This function is triggered once an event
    /// with a time tag later than the last possible voxel is detected. It
    /// currently updates the exisitng data based on a guesstimation regarding
    /// data quality, i.e. we don't do any error checking what-so-ever, we
    /// simply trust in the data being not faulty.
    fn update_snake_for_next_frame(&mut self, next_frame_at: Picosecond) {
        if next_frame_at == self.earliest_frame_time {
            info!("Already updated the next frame");
            return;
        }
        self.last_accessed_idx = 0;
        let offset = next_frame_at - self.earliest_frame_time;
        for pair in self.data.iter_mut() {
            pair.end_time += offset;
        }
        self.max_frame_time = self.data[self.data.len() - 1].end_time;
        self.earliest_frame_time = next_frame_at;
        info!(
            "Done populating next frame, summary:\nmax_frame_time: {}\nearliest_frame: {}",
            self.max_frame_time, self.earliest_frame_time
        );
    }

    fn get_z_imagespace_planes(&self) -> Option<DVector<Coordinate>> {
        None
    }
}

/// A three-dimensional volume rendered in a snake
impl Snake for ThreeDimensionalSnake {
    fn from_acq_params(config: &AppConfig, offset: Picosecond) -> ThreeDimensionalSnake {
        let mut threed_snake = ThreeDimensionalSnake::naive_init(config);
        threed_snake.data = threed_snake.allocate_snake(&config);
        threed_snake.data.push(TimeCoordPair::new(
            offset,
            ImageCoor::new(
                OrderedFloat(f32::NAN),
                OrderedFloat(f32::NAN),
                OrderedFloat(f32::NAN),
            ),
        ));
        let num_columns = config.columns as usize;
        let mut column_deltas_ps =
            threed_snake.construct_row_ps_snake(num_columns, &threed_snake.voxel_delta_ps);
        let column_deltas_imagespace =
            threed_snake.construct_row_im_snake(num_columns, &threed_snake.voxel_delta_im);
        match config.bidir {
            Bidirectionality::Bidir => threed_snake.update_naive_with_parameters_bidir(
                &config,
                &mut column_deltas_ps,
                &column_deltas_imagespace,
                offset,
            ),
            Bidirectionality::Unidir => threed_snake.update_naive_with_parameters_unidir(
                &config,
                &mut column_deltas_ps,
                &column_deltas_imagespace,
                offset,
            ),
        }
    }

    fn get_voxel_delta_im(&self) -> VoxelDelta<Coordinate> {
        self.voxel_delta_im.clone()
    }

    fn calc_snake_length(&self, config: &AppConfig) -> usize {
        let baseline_count = ((config.columns + 1) * config.rows) as usize;
        baseline_count * config.planes as usize + 1
    }

    fn time_to_coord_linear(&mut self, time: i64, ch: usize) -> ProcessedEvent {
        if time > self.max_frame_time {
            debug!(
                "Photon arrived after end of Frame! Our time: {}, Max time: {}",
                time, self.max_frame_time
            );
            self.update_snake_for_next_frame(self.max_frame_time + self.frame_dead_time);
            return ProcessedEvent::PhotonNewFrame;
        }
        let mut additional_steps_taken = 0usize;
        let mut coord = None;
        for pair in &self.data[self.last_accessed_idx..] {
            if time <= pair.end_time {
                self.last_accessed_idx += additional_steps_taken;
                coord = Some(self.update_z_coord(pair.coord, time));
                break;
            }
            additional_steps_taken += 1;
        }
        // Makes sure that we indeed captured some cell. This can be avoided in
        // principle but I'm still not confident enough in this implementation.
        if let Some(coord) = coord {
            trace!("Found a point on the snake! Time: {}; Additional steps taken: {}; Channel: {}. The coord we're sending is: {:?}", time, additional_steps_taken, ch, coord);
            ProcessedEvent::Displayed(coord, ch)
        } else {
            error!(
                "Coordinate remained unpopulated. self.data: {:?}\nAdditional steps taken: {}",
                &self.data[self.last_accessed_idx..],
                additional_steps_taken
            );
            panic!("Coordinate remained unpopulated for some reason. Investigate!")
        }
    }

    fn update_z_coord(&self, coord: ImageCoor, time: Picosecond) -> ImageCoor {
        let tag_delta = time - self.last_taglens_time;
        ImageCoor::new(coord.x, coord.y, self.tag_deltas_to_coord[tag_delta])
    }

    fn update_snake_for_next_frame(&mut self, next_frame_at: Picosecond) {
        self.last_accessed_idx = 0;
        let offset = next_frame_at - self.earliest_frame_time;
        for pair in self.data.iter_mut() {
            pair.end_time += offset;
        }
        self.max_frame_time = self.data[self.data.len() - 1].end_time;
        self.earliest_frame_time = next_frame_at;
        info!(
            "Done populating next frame, summary:\nmax_frame_time: {}\nearliest_frame: {}",
            self.max_frame_time, self.earliest_frame_time
        );
    }

    fn get_earliest_frame_time(&self) -> Picosecond {
        self.earliest_frame_time
    }

    fn new_taglens_period(&mut self, time: Picosecond) -> ProcessedEvent {
        self.last_taglens_time = time;
        ProcessedEvent::NoOp
    }

    fn get_z_imagespace_planes(&self) -> Option<DVector<Coordinate>> {
        Some(self.tag_deltas_to_coord.get_im_vec())
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;
    use crate::configuration::{AppConfigBuilder, InputChannel, Period};

    /// Helper method to test config-dependent things without actually caring
    /// about the different config values
    fn setup_default_config() -> AppConfigBuilder {
        AppConfigBuilder::default()
            .with_laser_period(Period::from_freq(80_000_000.0))
            .with_rows(256)
            .with_columns(256)
            .with_planes(10)
            .with_scan_period(Period::from_freq(7926.17))
            .with_tag_period(Period::from_freq(189800))
            .with_bidir(Bidirectionality::Bidir)
            .with_fill_fraction(71.3)
            .with_frame_dead_time(8 * *Period::from_freq(7926.17))
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(0, 0.0))
            .with_pmt3_ch(InputChannel::new(0, 0.0))
            .with_pmt4_ch(InputChannel::new(0, 0.0))
            .with_laser_ch(InputChannel::new(0, 0.0))
            .with_frame_ch(InputChannel::new(0, 0.0))
            .with_line_ch(InputChannel::new(2, 0.0))
            .with_taglens_ch(InputChannel::new(3, 0.0))
            .with_line_shift(0)
            .clone()
    }

    fn setup_image_scanning_config() -> AppConfigBuilder {
        AppConfigBuilder::default()
            .with_laser_period(Period::from_freq(80_000_000.0))
            .with_rows(10)
            .with_columns(10)
            .with_planes(1)
            .with_scan_period(Period::from_freq(1_000_000_000))
            .with_tag_period(Period::from_freq(189800))
            .with_bidir(Bidirectionality::Bidir)
            .with_fill_fraction(50i16)
            .with_frame_dead_time(1 * *Period::from_freq(1_000_000_000))
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(0, 0.0))
            .with_pmt3_ch(InputChannel::new(0, 0.0))
            .with_pmt4_ch(InputChannel::new(0, 0.0))
            .with_laser_ch(InputChannel::new(0, 0.0))
            .with_frame_ch(InputChannel::new(0, 0.0))
            .with_line_ch(InputChannel::new(2, 0.0))
            .with_taglens_ch(InputChannel::new(0, 0.0))
            .with_line_shift(0)
            .clone()
    }

    fn naive_init_2d(config: &AppConfig) -> TwoDimensionalSnake {
        TwoDimensionalSnake::naive_init(config)
    }

    fn naive_init_3d(config: &AppConfig) -> ThreeDimensionalSnake {
        ThreeDimensionalSnake::naive_init(config)
    }

    #[test]
    fn test_tag_period_freq_conversion() {
        let freq = 1; // Hz
        assert_eq!(Period::from_freq(freq).period, 1_000_000_000_000);
    }

    #[test]
    fn test_tag_period_normal_freq() {
        let freq = 189_800; // Hz
        assert_eq!(Period::from_freq(freq).period, 5_268_704);
    }

    #[test]
    fn test_period_to_hz() {
        let period = 1_000_000_000_000i64;
        assert_eq!(Period { period: period }.to_hz(), 1.0f32);
    }

    #[test]
    fn test_period_to_hz_smaller() {
        let period = 1_000_000_000i64;
        assert_eq!(Period { period: period }.to_hz(), 1000.0f32);
    }

    #[test]
    fn voxel_delta_columns_standard() {
        let config = setup_default_config().build();
        assert_eq!(VoxelDelta::calc_time_between_columns(&config), 175_693);
    }

    #[test]
    fn voxel_delta_effective_line_period() {
        let config = setup_default_config().build();
        assert_eq!(VoxelDelta::calc_effective_line_period(&config), 44_977_590);
    }

    #[test]
    fn voxel_delta_between_planes() {
        let config = setup_default_config().build();
        assert_eq!(VoxelDelta::calc_time_between_planes(&config), 263_435);
    }

    #[test]
    fn voxel_delta_default_config_calcs() {
        let config = setup_default_config().build();
        let voxel_delta = VoxelDelta {
            row: 18_104_579,
            column: 175_693,
            plane: 263_435,
            frame: 1_009_314_712,
            volsize: VolumeSize::from_config(&config),
            num_planes: config.planes,
        };
        assert_eq!(VoxelDelta::<Picosecond>::from_config(&config), voxel_delta)
    }

    #[test]
    fn voxel_delta_time_between_rows() {
        let config = setup_default_config()
            .with_bidir(Bidirectionality::Bidir)
            .build();
        assert_eq!(VoxelDelta::calc_time_between_rows(&config), 18_104_579);
    }

    #[test]
    fn voxel_delta_imcoord_config() {
        let config = setup_default_config()
            .with_rows(3)
            .with_columns(5)
            .with_planes(2)
            .build();
        let vd = VoxelDelta::<Coordinate>::from_config(&config);
        assert_eq!(vd.row, RENDERING_SPAN / OrderedFloat(2.0));
        assert_eq!(vd.column, RENDERING_SPAN / OrderedFloat(4.0));
        assert_eq!(vd.plane, RENDERING_SPAN);
    }

    #[test]
    fn voxel_delta_im_map_coord_2d_default() {
        let config = setup_default_config().with_rows(5).build();
        let vd = VoxelDelta::<Coordinate>::from_config(&config);
        let result = VoxelDelta::create_single_coord_idx_mapping(config.rows, vd.row);
        let mut truth = BTreeMap::new();
        let floats = vec![-0.5f32, -0.25, 0.0, 0.25, 0.5];
        for (idx, float) in (0..5).zip(floats.iter()) {
            truth.insert(OrderedFloat(*float), idx as u32);
        }
        assert_eq!(result, truth);
    }

    #[test]
    fn voxel_delta_im_map_coord_2d_check_planes() {
        let config = setup_default_config().with_rows(5).with_planes(1).build();
        let vd = VoxelDelta::<Coordinate>::from_config(&config);
        let result = vd.map_coord_to_index();
        let mut truth = BTreeMap::new();
        let floats = vec![0.0f32];
        for (idx, float) in (0..1).zip(floats.iter()) {
            truth.insert(OrderedFloat(*float), idx as u32);
        }
        assert_eq!(result.0, truth);
    }

    #[test]
    fn voxel_delta_im_map_coord_3d_default() {
        let config = setup_default_config().with_planes(12).build();
        let vd = VoxelDelta::<Coordinate>::from_config(&config);
        let result = VoxelDelta::create_single_coord_idx_mapping(config.planes, vd.plane);
        let mut truth = BTreeMap::new();
        let floats = vec![
            -0.5f32, -0.4, -0.3, -0.2, -0.1, 0.0, 0.1, 0.2, 0.3, 0.4, 0.5,
        ];
        for (idx, float) in (0..10).zip(floats.iter()) {
            truth.insert(OrderedFloat(*float), idx as u32);
        }

        let _ = result.iter().zip(truth.iter()).map(|(x, y)| {
            assert_approx_eq!(x.0.into_inner(), y.0.into_inner(), 0.001f32);
            assert_eq!(x.1, y.1);
        });
    }

    #[test]
    fn voxel_delta_im_map_coord_3d_two_planes() {
        let config = setup_default_config().with_planes(2).build();
        let vd = VoxelDelta::<Coordinate>::from_config(&config);
        let result = VoxelDelta::create_single_coord_idx_mapping(config.planes, vd.plane);
        let mut truth = BTreeMap::new();
        let floats = vec![-0.5f32, 0.5];
        for (idx, float) in (0..2).zip(floats.iter()) {
            truth.insert(OrderedFloat(*float), idx as u32);
        }

        let _ = result.iter().zip(truth.iter()).map(|(x, y)| {
            assert_approx_eq!(x.0.into_inner(), y.0.into_inner(), 0.001f32);
            assert_eq!(x.1, y.1);
        });
    }

    #[test]
    fn test_convert_fillfrac_unidir_to_deadtime() {
        let config = setup_default_config()
            .with_bidir(Bidirectionality::Unidir)
            .build();
        assert_eq!(VoxelDelta::calc_time_between_rows(&config), 99_291_327);
    }

    #[test]
    fn time_to_coord_snake_2d_bidir() {
        let config = setup_image_scanning_config()
            .with_bidir(Bidirectionality::Bidir)
            .build();
        let snake = TwoDimensionalSnake::from_acq_params(&config, 0);
        assert_eq!(
            snake.data[1],
            TimeCoordPair::new(
                25,
                ImageCoor::new(RENDERING_BOUNDS.0, RENDERING_BOUNDS.0, RENDERING_BOUNDS.1)
            ),
        );
        assert_eq!(
            snake.data[12],
            TimeCoordPair::new(
                525,
                ImageCoor::new(
                    RENDERING_BOUNDS.0 + (RENDERING_SPAN / OrderedFloat(9.0f32)),
                    RENDERING_BOUNDS.2,
                    RENDERING_BOUNDS.1,
                )
            ),
        );
        assert_eq!(
            snake.data[35],
            TimeCoordPair::new(
                1550,
                ImageCoor::new(
                    RENDERING_BOUNDS.0
                        + OrderedFloat(3.0) * (RENDERING_SPAN / OrderedFloat(9.0f32)),
                    RENDERING_BOUNDS.2 - (RENDERING_SPAN / OrderedFloat(9.0f32)),
                    RENDERING_BOUNDS.1,
                )
            ),
        );
        assert_eq!(
            snake.data[snake.data.len() - 1],
            TimeCoordPair::new(
                4750,
                ImageCoor::new(RENDERING_BOUNDS.2, RENDERING_BOUNDS.0, RENDERING_BOUNDS.1)
            )
        );
        assert_eq!(snake.data.len() + 1, snake.data.capacity());
    }

    #[test]
    fn time_to_coord_snake_2d_unidir() {
        let config = setup_image_scanning_config()
            .with_bidir(Bidirectionality::Unidir)
            .build();
        let snake = TwoDimensionalSnake::from_acq_params(&config, 0);
        assert_eq!(
            snake.data[1],
            TimeCoordPair::new(
                25,
                ImageCoor::new(RENDERING_BOUNDS.0, RENDERING_BOUNDS.0, RENDERING_BOUNDS.1)
            ),
        );
        assert_eq!(
            snake.data[12],
            TimeCoordPair::new(
                1275,
                ImageCoor::new(
                    RENDERING_BOUNDS.0 + (RENDERING_SPAN / OrderedFloat(9.0f32)),
                    RENDERING_BOUNDS.0,
                    RENDERING_BOUNDS.1
                )
            ),
        );
        assert_eq!(
            snake.data[35],
            TimeCoordPair::new(
                3800,
                ImageCoor::new(
                    RENDERING_BOUNDS.0
                        + OrderedFloat(3.0) * (RENDERING_SPAN / OrderedFloat(9.0f32)),
                    RENDERING_BOUNDS.0 + (RENDERING_SPAN / OrderedFloat(9.0f32)),
                    RENDERING_BOUNDS.1
                )
            ),
        );
        assert_eq!(
            snake.data[snake.data.len() - 1],
            TimeCoordPair::new(
                11500,
                ImageCoor::new(RENDERING_BOUNDS.2, RENDERING_BOUNDS.2, RENDERING_BOUNDS.1)
            )
        );
        assert_eq!(snake.data.len() + 1, snake.data.capacity());
        assert_eq!(snake.data.len() + 1, snake.data.capacity());
    }

    #[test]
    fn snake_2d_metadata_bidir() {
        let config = setup_image_scanning_config().with_bidir(true).build();
        let twod_snake = naive_init_2d(&config);
        let column_deltas_ps =
            twod_snake.construct_row_ps_snake(config.columns as usize, &twod_snake.voxel_delta_ps);
        let column_deltas_im =
            twod_snake.construct_row_im_snake(config.columns as usize, &twod_snake.voxel_delta_im);
        assert_eq!(column_deltas_ps.len(), 11);
        assert_eq!(column_deltas_im.len(), 11);
        let last_idx = column_deltas_im.len() - 1;
        assert_eq!(
            column_deltas_ps[last_idx] - column_deltas_ps[last_idx - 1],
            twod_snake.voxel_delta_ps.row
        );
    }

    #[test]
    fn snake_2d_metadata_unidir() {
        let config = setup_image_scanning_config().with_bidir(false).build();
        let twod_snake = naive_init_2d(&config);
        let column_deltas_ps =
            twod_snake.construct_row_ps_snake(config.columns as usize, &twod_snake.voxel_delta_ps);
        let column_deltas_im =
            twod_snake.construct_row_im_snake(config.columns as usize, &twod_snake.voxel_delta_im);
        assert_eq!(column_deltas_ps.len(), 11);
        assert_eq!(column_deltas_im.len(), 11);
        let last_idx = column_deltas_im.len() - 1;
        assert_eq!(
            column_deltas_ps[last_idx] - column_deltas_ps[last_idx - 1],
            twod_snake.voxel_delta_ps.row
        );
    }

    #[test]
    fn build_snake_2d() {
        let config = setup_image_scanning_config().build();
        let twod_snake = naive_init_2d(&config);
        let snake = twod_snake.allocate_snake(&config);
        assert_eq!(snake.capacity(), 111);
    }

    #[test]
    fn build_snake_3d() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let threed_snake = naive_init_3d(&config);
        let snake = threed_snake.allocate_snake(&config);
        assert_eq!(snake.capacity(), 1101);
    }

    #[test]
    /// Numpy code that creates these truth vectors:
    /// def create_plane_coords(planes) -> np.ndarray:
    ///     elems = int(planes / 2)
    ///     q2_coords = np.linspace(0.5, -0.5, planes)
    ///     q1_coords = np.flip(q2_coords[1:elems])
    ///     q3_coords = np.flip(q2_coords[elems:-1])
    ///     return np.concatenate([q1_coords, q2_coords, q3_coords])
    fn create_sine_imagespace_many_planes() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let snake = ThreeDimensionalSnake::naive_init(&config);
        let sine = snake.create_planes_snake_imagespace(config.planes as usize);
        let truth: DVector<f32> = DVector::from_vec(vec![
            0.05555556f32,
            0.16666667,
            0.27777778,
            0.38888889,
            0.5,
            0.38888889,
            0.27777778,
            0.16666667,
            0.05555556,
            -0.05555556,
            -0.16666667,
            -0.27777778,
            -0.38888889,
            -0.5,
            -0.38888889,
            -0.27777778,
            -0.16666667,
            -0.05555556,
        ]);
        let _: Vec<_> = sine
            .iter()
            .zip(truth.iter())
            .map(|x| assert_approx_eq!(x.0.into_inner(), x.1))
            .collect();
    }

    #[should_panic]
    #[test]
    fn create_sine_imagespace_uneven_planes() {
        let _ = setup_image_scanning_config().with_planes(13).build();
    }

    #[test]
    /// Numpy code that create these truth vectors:
    /// def calc_asin_ps(length, period) -> np.ndarray:
    ///     coords = create_plane_coords(length)
    ///     asin = np.arcsin(coords * 2.0) / (np.pi / 2)
    ///     ps = np.full(len(coords), period / 4)
    ///     firstq = int(length / 2) - 1
    ///     ps[:firstq] *= asin[:firstq]
    ///     sec_slice = slice(firstq, firstq+length)
    ///     ps[sec_slice] = ps[sec_slice] * (1.0 - asin[sec_slice]) + (period / 4)
    ///     last_slice = slice(firstq+length, len(coords))
    ///     ps[last_slice] = ps[last_slice] * (1.0 + asin[last_slice]) + (3 * period / 4)
    ///     return ps.astype(np.int64)
    fn create_sine_ps_no_offset() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let snake = ThreeDimensionalSnake::naive_init(&config);
        let planes = config.planes as usize;
        let sine = snake.create_planes_snake_imagespace(planes);
        let sine_ps = snake.create_planes_snake_ps(&sine, 1000);
        let truth = DVector::from_vec(vec![
            17i64, 54, 93, 141, 250, 358, 406, 445, 482, 517, 554, 593, 641, 750, 858, 906, 945,
            982,
        ]);
        let c = sine_ps
            .iter()
            .zip(truth.iter())
            .filter(|(a, b)| a == b)
            .count();
        assert_eq!(c, sine_ps.len());
    }

    #[test]
    #[should_panic]
    fn setup_interval_coord_map_incorrectly() {
        let im = DVector::from_vec(
            vec![-1.0f32, -0.5, 0.0, 0.5, 1.0]
                .iter()
                .map(|x| OrderedFloat(*x))
                .collect(),
        );
        let time = DVector::from_vec(vec![0i64, 10, 20, 30, 40, 50]);
        IntervalToCoordMap::new(im, time);
    }

    fn setup_interval_coord_map_correctly() -> IntervalToCoordMap {
        let im = DVector::from_vec(
            vec![-0.5f32, -0.3, -0.1, 0.1, 0.3, 0.5]
                .iter()
                .map(|x| OrderedFloat(*x))
                .collect(),
        );
        let time = DVector::from_vec(vec![0i64, 10, 20, 30, 40, 50]);
        IntervalToCoordMap::new(im, time)
    }

    #[test]
    fn interval_index() {
        let interval = setup_interval_coord_map_correctly();
        assert_eq!(interval[9], -0.3f32);
        assert_eq!(interval[50], 0.5f32);
        assert_eq!(interval[500], 0.0f32);
    }
}
