extern crate log;
use std::f32::consts::PI;

use nalgebra::{Const, DVector, SVector};
use serde::{Deserialize, Serialize};

use crate::configuration::{AppConfig, Bidirectionality};
use crate::point_cloud_renderer::{ImageCoor, ProcessedEvent};
use crate::DISPLAY_COLORS;

/// TimeTagger absolute times are i64 values that represent the number of
/// picoseconds since the start of the experiment
pub type Picosecond = i64;

/// Marker trait to allow specific types to be used as deltas between pixels -
/// for the image space rendering case the deltas are in f32, while for the
/// rendering the deltas are in Picoseconds.
pub trait ImageDelta {}

impl ImageDelta for f32 {}
impl ImageDelta for Picosecond {}

/// Data regarding the step size, either in image space or in picoseconds, that
/// is needed to construct the 'snake' data vector of [`TimeToCoord`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoxelDelta<T: ImageDelta> {
    column: T,
    row: T,
    plane: T,
    frame: T,
}

impl VoxelDelta<f32> {
    pub(crate) fn from_config(config: &AppConfig) -> VoxelDelta<f32> {
        let jump_between_columns = 2.0f32 / (config.columns as f32 - 1.0);
        let jump_between_rows = 2.0f32 / (config.rows as f32 - 1.0);
        let jump_between_planes: f32;
        if config.planes > 1 {
            jump_between_planes = 2.0f32 / (config.planes as f32 - 1.0);
        } else {
            jump_between_planes = 2.0;
        }

        VoxelDelta {
            column: jump_between_columns,
            row: jump_between_rows,
            plane: jump_between_planes,
            frame: f32::NAN,
        }
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

    fn min(config: &AppConfig) -> Picosecond {
        let interim = [
            VoxelDelta::calc_time_between_rows(config),
            VoxelDelta::calc_time_between_columns(config),
            VoxelDelta::calc_time_between_planes(config),
        ];
        let vals = interim.iter().min();
        *vals.unwrap()
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
        let end_of_rotation_value = column_deltas_ps[(num_columns - 1)] + voxel_delta_ps.row;
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
        voxel_delta_im: &VoxelDelta<f32>,
    ) -> DVector<f32> {
        let column_deltas_imagespace =
            DVector::<f32>::from_fn(num_columns, |i, _| ((i as f32) * voxel_delta_im.column));
        // The events during mirror rotation will be discarded - The NaN takes
        // care of that
        let column_deltas_imagespace =
            column_deltas_imagespace
                .add_scalar(-1.0)
                .insert_rows(num_columns, 1, f32::NAN);
        column_deltas_imagespace
    }

    /// Generate an imagespace row snake for the bidirectional rows.
    ///
    /// The odd rows should have the order of the cells in their snakes
    /// reversed.
    fn reverse_row_imagespace(&self, column_deltas_imagespace: &DVector<f32>) -> DVector<f32> {
        let mut column_deltas_imagespace_rev: Vec<f32> = (column_deltas_imagespace
            .iter()
            .rev()
            .copied()
            .collect::<Vec<f32>>())
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
    fn time_to_coord_linear(&mut self, time: i64, ch: usize) -> ProcessedEvent;

    /// Handles a new TAG lens start-of-cycle event
    fn new_taglens_period(&self, _time: i64) -> ProcessedEvent {
        ProcessedEvent::NoOp
    }

    fn new_laser_event(&self, _time: i64) -> ProcessedEvent {
        ProcessedEvent::NoOp
    }

    fn dump(&self, _time: i64) -> ProcessedEvent {
        ProcessedEvent::NoOp
    }
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
    voxel_delta_im: VoxelDelta<f32>,
    /// The earliest time of the first voxel
    earliest_frame_time: Picosecond,
    /// The time it takes the software to finish a full frame, not including
    /// dead time between frames
    frame_duration: Picosecond,
}

pub struct ThreeDimensionalSnake {
    /// A vector of end times with their corresponding image-space
    /// coordinates.
    data: Vec<TimeCoordPair>,
    /// The index in data that was last used to retrieve a
    /// coordinates. We keep it to look for the next matching end time only from
    /// that value onward.
    last_accessed_idx: usize,
    last_taglens_time: Picosecond,
    /// The end time for the frame. Useful to quickly check
    /// whether a time tag belongs in the next frame.
    max_frame_time: Picosecond,
    /// Deltas in ps of consecutive pixels, lines, etc.
    voxel_delta_ps: VoxelDelta<Picosecond>,
    /// Deltas in image space of consecutive pixels, lines, etc.
    voxel_delta_im: VoxelDelta<f32>,
    /// The earliest time of the first voxel
    earliest_frame_time: Picosecond,
    /// The time it takes the software to finish a full frame, not including
    /// dead time between frames
    frame_duration: Picosecond,
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
    fn naive_init(config: &AppConfig) -> Self {
        let voxel_delta_ps = VoxelDelta::<Picosecond>::from_config(&config);
        let voxel_delta_im = VoxelDelta::<f32>::from_config(&config);

        Self {
            data: Vec::new(),
            voxel_delta_ps,
            voxel_delta_im,
            last_accessed_idx: 0,
            max_frame_time: 0,
            earliest_frame_time: 0,
            frame_duration: 0,
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
        column_deltas_imagespace: &DVector<f32>,
        offset: Picosecond,
    ) -> TwoDimensionalSnake {
        // Add the cell capturing all photons arriving between frames
        let deadtime_during_rotation = column_deltas_ps[column_deltas_ps.len() - 1];
        let mut line_offset: Picosecond = offset;
        let column_deltas_imagespace_rev = self.reverse_row_imagespace(column_deltas_imagespace);
        let column_deltas_ps_bidir = self.reverse_row_picosecond(column_deltas_ps, config.line_shift);
        let mut row_coord: f32;
        for row in (0..config.rows).step_by(2) {
            // Start with the unidir row
            row_coord = ((row as f32) * self.voxel_delta_im.row) - 1.0;
            TwoDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace,
                &column_deltas_ps,
                row_coord,
                line_offset,
            );
            line_offset += deadtime_during_rotation;
            // Now the bidir row
            row_coord = (((row + 1) as f32) * self.voxel_delta_im.row) - 1.0;
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
            frame_duration: config.calc_frame_duration(),
        }
    }

    /// Update the time -> coordinate snake when we're scanning unidirectionally.
    ///
    /// This method is also used in the bidirectional case, except it's used
    /// once for the even ones and again for the odd ones with different
    /// paraneters.
    fn push_pair_unidir(
        snake: &mut Vec<TimeCoordPair>,
        column_deltas_imagespace: &DVector<f32>,
        column_deltas_ps: &DVector<Picosecond>,
        row_coord: f32,
        line_offset: Picosecond,
    ) {
        for (column_delta_im, column_delta_ps) in column_deltas_imagespace
            .into_iter()
            .zip(column_deltas_ps.into_iter())
        {
            let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, 0.0);
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
        column_deltas_imagespace: &DVector<f32>,
        offset: Picosecond,
    ) -> TwoDimensionalSnake {
        // Add the cell capturing all photons arriving between frames
        let line_len = column_deltas_ps.len();
        let offset_per_row = column_deltas_ps[line_len - 1];
        let mut line_offset: Picosecond = offset;
        for row in 0..config.rows {
            let row_coord = ((row as f32) * self.voxel_delta_im.row) - 1.0;
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
        let frame_duration = config.calc_frame_duration();
        info!("2D unidir snake finished");
        TwoDimensionalSnake {
            data: self.data,
            last_accessed_idx: 0,
            max_frame_time,
            voxel_delta_ps: self.voxel_delta_ps,
            voxel_delta_im: self.voxel_delta_im,
            earliest_frame_time: offset,
            frame_duration,
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
        let voxel_delta_im = VoxelDelta::<f32>::from_config(&config);

        Self {
            data: Vec::new(),
            voxel_delta_ps,
            voxel_delta_im,
            last_accessed_idx: 0,
            last_taglens_time: 0,
            max_frame_time: 0,
            earliest_frame_time: 0,
            frame_duration: 0,
        }
    }

    fn push_pair_unidir(
        snake: &mut Vec<TimeCoordPair>,
        column_deltas_imagespace: &DVector<f32>,
        column_deltas_ps: &DVector<Picosecond>,
        row_coord: f32,
        line_offset: Picosecond,
    ) {
        todo!()
    }

    fn populate_snake(&mut self, starting_coord: ImageCoor, config: &AppConfig) {
        let smallest_time_unit = VoxelDelta::min(config);
        let mut snake: Vec<TimeCoordPair> =
            Vec::with_capacity((config.planes * config.columns * config.rows) as usize);
        let planes_im = self.create_planes_snake_imagespace(config.planes as usize);
        let planes_ps =
            self.create_planes_snake_ps(&planes_im, *config.tag_period, self.earliest_frame_time);
        snake
            .iter_mut()
            .zip(planes_im.iter().zip(planes_ps.iter()))
            .map(|x| x.1);
        // let mut ordered = OrderedCoordinates::new(config);
        // let time = self.earliest_frame_time;
        // while time < self.max_frame_time {
        //     while time < ordered.slow.delta {
        //         while time < ordered.mid.delta {
        //             snake.push(TimeCoordPair::new(time, ordered.coord));
        //             ordered.fast.next();
        //             trace!("Added coord on time {} and place {}", time, ordered.coord);
        //             time += ordered.fast.delta;
        //         }
        //         ordered.mid.next();
        //         time = time % ordered.mid.delta;
        // }
        // ordered.slow.next();
        // }
        self.data = snake;
    }

    fn create_planes_snake_imagespace(&self, planes: usize) -> DVector<f32> {
        let step_size = 2.0f32 / (planes as f32);
        let half_planes = planes / 2;
        let mut phase_limits_0_to_1 =
            DVector::<f32>::from_fn(half_planes, |i, _| (i as f32) * step_size);
        let mut phase_limits_1_to_m1 =
            DVector::<f32>::from_fn(planes, |i, _| ((i + 1) as f32) * (-step_size))
                .iter()
                .rev()
                .copied()
                .collect::<Vec<f32>>();
        let mut phase_limits_m1_to_0 = DVector::<f32>::from_fn(half_planes - 1, |i, _| {
            ((i - 1) as f32) * step_size + step_size
        });
        let mut phase_limits_0_to_1 = phase_limits_0_to_1.as_mut_slice().to_vec();
        phase_limits_0_to_1.append(&mut phase_limits_1_to_m1);
        phase_limits_0_to_1.append(&mut phase_limits_m1_to_0.as_mut_slice().to_vec());
        let phase_limits = DVector::<f32>::from_vec(phase_limits_0_to_1);
        phase_limits
    }

    fn create_planes_snake_ps(
        &self,
        planes: &DVector<f32>,
        delta: Picosecond,
        offset: Picosecond,
    ) -> DVector<Picosecond> {
        let quarter_delta = (delta / 4) as f32;
        let num_planes = planes.len();
        let mut asin = planes.map(|x| x.asin() / (PI / 2.0));
        let mut sine_ps = DVector::<f32>::repeat(num_planes, quarter_delta);
        sine_ps
            .rows_mut(0, num_planes / 4)
            .component_mul(&asin.rows(0, num_planes / 4));
        sine_ps
            .rows_mut(num_planes / 4, num_planes / 2)
            .component_mul(
                &asin
                    .rows_mut(num_planes / 4, num_planes / 2)
                    .map(|x| 1.0 - x),
            )
            .add_scalar_mut(quarter_delta);
        sine_ps
            .rows_mut(3 * num_planes / 4, num_planes / 4)
            .component_mul(
                &asin
                    .rows_mut(3 * num_planes / 4, num_planes / 4)
                    .map(|x| 1.0 + x),
            )
            .add_scalar_mut(3.0 * quarter_delta);
        sine_ps.map(|x| (x as Picosecond) + offset)
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
        column_deltas_imagespace: &DVector<f32>,
        offset: Picosecond,
    ) -> Self {
        // Add the cell capturing all photons arriving between frames
        let deadtime_during_rotation = column_deltas_ps[column_deltas_ps.len() - 1];
        let mut line_offset: Picosecond = offset;
        let column_deltas_imagespace_rev = self.reverse_row_imagespace(column_deltas_imagespace);
        let column_deltas_ps_bidir = self.reverse_row_picosecond(column_deltas_ps, -2000000);
        let mut row_coord: f32;
        for row in (0..config.rows).step_by(2) {
            // Start with the unidir row
            row_coord = ((row as f32) * self.voxel_delta_im.row) - 1.0;
            ThreeDimensionalSnake::push_pair_unidir(
                &mut self.data,
                &column_deltas_imagespace,
                &column_deltas_ps,
                row_coord,
                line_offset,
            );
            line_offset += deadtime_during_rotation;
            // Now the bidir row
            row_coord = (((row + 1) as f32) * self.voxel_delta_im.row) - 1.0;
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
        info!("3D bidir Snake built");
        ThreeDimensionalSnake {
            data: self.data,
            last_accessed_idx: 0,
            last_taglens_time: 0,
            max_frame_time,
            voxel_delta_ps: self.voxel_delta_ps,
            voxel_delta_im: self.voxel_delta_im,
            earliest_frame_time: offset,
            frame_duration: config.calc_frame_duration(),
        }
    }
}

impl Snake for TwoDimensionalSnake {
    fn from_acq_params(config: &AppConfig, offset: Picosecond) -> TwoDimensionalSnake {
        let mut twod_snake = TwoDimensionalSnake::naive_init(config);
        twod_snake.data = twod_snake.allocate_snake(&config);
        twod_snake.data.push(TimeCoordPair::new(
            offset,
            ImageCoor::new(f32::NAN, f32::NAN, f32::NAN),
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

    fn get_earliest_frame_time(&self) -> Picosecond {
        self.earliest_frame_time
    }

    /// Returns the value assigned to the snake's capacity
    ///
    /// For 2D imaging it's num_rows * (num_columns + 1), and for 3D we add
    /// in the number of planes.
    ///
    /// These numbers take into account a cell before each frame which captures
    /// photons arriving between frames, and a cell we remove from the last row
    /// which is not needed and a cell that is added so that we don't over-
    /// allocate..
    fn calc_snake_length(&self, config: &AppConfig) -> usize {
        let baseline_count = ((config.columns + 1) * config.rows) as usize;
        match config.planes {
            0 | 1 => baseline_count + 1,
            _ => baseline_count * config.planes as usize + 1,
        }
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
            ProcessedEvent::Displayed(coord, DISPLAY_COLORS[ch])
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
    /// This function is triggered from the 'tag_to_coord' method once an event
    /// with a time tag later than the last possible voxel is detected. It
    /// currently updates the exisitng data based on a guesstimation regarding
    /// data quality, i.e. we don't do any error checking what-so-ever, we
    /// simply trust in the data being not faulty.
    fn update_snake_for_next_frame(&mut self, next_frame_at: Picosecond) {
        self.last_accessed_idx = 0;
        let offset = next_frame_at - self.earliest_frame_time;
        for pair in self.data.iter_mut() {
            pair.end_time += offset;
        }
        self.max_frame_time = self.data[self.data.len() - 1].end_time;
        self.earliest_frame_time = next_frame_at;
        info!("Done populating next frame, summary:\nmax_frame_time: {}\nearliest_frame: {}\nframe_duration: {}", self.max_frame_time,self.earliest_frame_time, self.frame_duration);
    }
}

/// A three-dimensional volume rendered in a snake
impl Snake for ThreeDimensionalSnake {
    fn from_acq_params(config: &AppConfig, offset: Picosecond) -> ThreeDimensionalSnake {
        let mut threed_snake = ThreeDimensionalSnake::naive_init(config);
        threed_snake.data = threed_snake.allocate_snake(&config);
        threed_snake.data.push(TimeCoordPair::new(
            offset,
            ImageCoor::new(f32::NAN, f32::NAN, f32::NAN),
        ));
        let num_columns = config.columns as usize;
        let mut column_deltas_ps =
            threed_snake.construct_row_ps_snake(num_columns, &threed_snake.voxel_delta_ps);
        let column_deltas_imagespace =
            threed_snake.construct_row_im_snake(num_columns, &threed_snake.voxel_delta_im);
        let planes_delta_ps =
            threed_snake.construct_row_im_snake(num_columns, &threed_snake.voxel_delta_im);
        let planes_delta_im =
            threed_snake.construct_row_im_snake(num_columns, &threed_snake.voxel_delta_im);
        todo!()
        // match config.bidir {
        //     Bidirectionality::Bidir => twod_snake.update_naive_with_parameters_bidir(
        //         &config,
        //         &mut column_deltas_ps,
        //         &column_deltas_imagespace,
        //         offset,), Bidirectionality::Unidir =>
        //         twod_snake.update_naive_with_parameters_unidir( &config, &mut column_deltas_ps,
        //         &column_deltas_imagespace, offset,), }
    }

    fn calc_snake_length(&self, config: &AppConfig) -> usize {
        todo!()
    }

    fn time_to_coord_linear(&mut self, time: i64, ch: usize) -> ProcessedEvent {
        if time > self.max_frame_time {
            debug!(
                "Photon arrived after end of Frame! Our time: {}, Max time: {}",
                time, self.max_frame_time
            );
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
            ProcessedEvent::Displayed(coord, DISPLAY_COLORS[ch])
        } else {
            error!(
                "Coordinate remained unpopulated. self.data: {:?}\nAdditional steps taken: {}",
                &self.data[self.last_accessed_idx..],
                additional_steps_taken
            );
            panic!("Coordinate remained unpopulated for some reason. Investigate!")
        }
    }

    fn update_snake_for_next_frame(&mut self, next_frame_at: Picosecond) {
        self.last_accessed_idx = 0;
        let offset = next_frame_at - self.earliest_frame_time;
        for pair in self.data.iter_mut() {
            pair.end_time += offset;
        }
        self.max_frame_time = self.data[self.data.len() - 1].end_time;
        self.last_taglens_time = 0;
        self.earliest_frame_time = next_frame_at;
        info!("Done populating next frame, summary:\nmax_frame_time: {}\nearliest_frame: {}\nframe_duration: {}", self.max_frame_time,self.earliest_frame_time, self.frame_duration);
    }

    fn get_earliest_frame_time(&self) -> Picosecond {
        self.earliest_frame_time
    }
}

struct OrderedCoordinates {
    ordered_snake: Vec<TimeCoordPair>,
}

impl OrderedCoordinates {
    pub fn new(
        config: &AppConfig,
        start_time: Picosecond,
        start_coord: ImageCoor,
        end_time: Picosecond,
    ) -> Self {
        let delta_ps = VoxelDelta::<Picosecond>::from_config(config);
        let delta_im = VoxelDelta::<f32>::from_config(config);
        let planes_vec = OrderedCoordinates::make_whole_frame_planes_vec_ps(
            delta_ps.plane,
            start_time,
            end_time,
        );
        todo!()
    }

    fn make_whole_frame_planes_vec_ps(
        delta: Picosecond,
        start_time: Picosecond,
        end_time: Picosecond,
    ) -> DVector<Picosecond> {
        let num_entries = (end_time - start_time) / delta; // output is floored automatically
        let deltas = DVector::<Picosecond>::from_fn(num_entries as usize, |i, _| {
            (i as Picosecond) * delta + delta
        });
        deltas
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::Point3;

    use super::*;
    use crate::configuration::{AppConfigBuilder, Period};

    /// Helper method to test config-dependent things without actually caring
    /// about the different config values
    fn setup_default_config() -> AppConfigBuilder {
        AppConfigBuilder::default()
            .with_point_color(Point3::new(1.0f32, 1.0, 1.0))
            .with_rows(256)
            .with_columns(256)
            .with_planes(10)
            .with_scan_period(Period::from_freq(7926.17))
            .with_tag_period(Period::from_freq(189800))
            .with_bidir(Bidirectionality::Bidir)
            .with_fill_fraction(71.3)
            .with_frame_dead_time(8 * *Period::from_freq(7926.17))
            .with_pmt1_ch(-1)
            .with_pmt2_ch(0)
            .with_pmt3_ch(0)
            .with_pmt4_ch(0)
            .with_laser_ch(0)
            .with_frame_ch(0)
            .with_line_ch(2)
            .with_taglens_ch(3)
            .with_line_shift(0)
            .clone()
    }

    fn setup_image_scanning_config() -> AppConfigBuilder {
        AppConfigBuilder::default()
            .with_point_color(Point3::new(1.0f32, 1.0, 1.0))
            .with_rows(10)
            .with_columns(10)
            .with_planes(1)
            .with_scan_period(Period::from_freq(1_000_000_000))
            .with_tag_period(Period::from_freq(189800))
            .with_bidir(Bidirectionality::Bidir)
            .with_fill_fraction(50i16)
            .with_frame_dead_time(1 * *Period::from_freq(1_000_000_000))
            .with_pmt1_ch(-1)
            .with_pmt2_ch(0)
            .with_pmt3_ch(0)
            .with_pmt4_ch(0)
            .with_laser_ch(0)
            .with_frame_ch(0)
            .with_line_ch(2)
            .with_taglens_ch(0)
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
        let vd = VoxelDelta::<f32>::from_config(&config);
        assert_eq!(vd.row, 1.0);
        assert_eq!(vd.column, 0.5);
        assert_eq!(vd.plane, 2.0);
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
        println!("{:?}", snake.data[..14].to_vec());
        assert_eq!(
            snake.data[1],
            TimeCoordPair::new(25, ImageCoor::new(-1.0, -1.0, 0.0)),
        );
        assert_eq!(
            snake.data[12],
            TimeCoordPair::new(525, ImageCoor::new(-1.0 + (2.0 / 9.0f32), 1.0, 0.0)),
        );
        assert_eq!(
            snake.data[35],
            TimeCoordPair::new(
                1550,
                ImageCoor::new(-1.0 + 3.0 * (2.0 / 9.0f32), 1.0 - (2.0 / 9.0f32), 0.0)
            ),
        );
        assert_eq!(
            snake.data[snake.data.len() - 1],
            TimeCoordPair::new(4750, ImageCoor::new(1.0, -1.0, 0.0))
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
            TimeCoordPair::new(25, ImageCoor::new(-1.0, -1.0, 0.0)),
        );
        assert_eq!(
            snake.data[12],
            TimeCoordPair::new(1275, ImageCoor::new(-1.0 + (2.0 / 9.0f32), -1.0, 0.0)),
        );
        assert_eq!(
            snake.data[35],
            TimeCoordPair::new(
                3800,
                ImageCoor::new(-1.0 + 3.0 * (2.0 / 9.0f32), -1.0 + (2.0 / 9.0f32), 0.0)
            ),
        );
        assert_eq!(
            snake.data[snake.data.len() - 1],
            TimeCoordPair::new(11500, ImageCoor::new(1.0, 1.0, 0.0))
        );
        assert_eq!(snake.data.len() + 1, snake.data.capacity());
        assert_eq!(snake.data.len() + 1, snake.data.capacity());
    }

    #[test]
    fn time_to_coord_snake_2d_first_item_has_offset() {
        let config = setup_image_scanning_config().build();
        let offset = 100;
        let snake = TwoDimensionalSnake::from_acq_params(&config, offset);
        assert_eq!(snake.data[0].end_time, offset);
        assert_eq!(
            snake.data[snake.data.len() - 1].end_time + snake.voxel_delta_ps.row,
            snake.frame_duration + offset
        );
    }

    // TODO: SECOND FRAMES' OFFSET?
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
        let twod_snake = naive_init_2d(&config);
        let snake = twod_snake.allocate_snake(&config);
        assert_eq!(snake.capacity(), 1101);
    }

    #[test]
    fn voxel_delta_min_2d() {
        let config = setup_image_scanning_config().with_planes(1).build();
        let min = VoxelDelta::min(&config);
        assert_eq!(min, 10);
    }

    #[test]
    fn voxel_delta_min_3d() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let min = VoxelDelta::min(&config);
        assert_eq!(min, 3);
    }

    #[test]
    fn create_sine_imagespace_single_plane() {
        let config = setup_image_scanning_config().with_planes(1).build();
        let snake = ThreeDimensionalSnake::naive_init(&config);
        let sine = snake.create_planes_snake_imagespace(config.planes as usize);
        assert_eq!(sine, DVector::from_vec(vec![0.0f32]));
    }

    #[test]
    fn create_sine_imagespace_many_planes() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let snake = ThreeDimensionalSnake::naive_init(&config);
        let sine = snake.create_planes_snake_imagespace(config.planes as usize);
        assert_eq!(
            sine,
            DVector::from_vec(vec![
                0.0f32, 0.2, 0.4, 0.6, 0.8, 1.0, 0.8, 0.6, 0.4, 0.2, 0.0, -0.2, -0.4, -0.6, -0.8,
                -1.0, -0.8, -0.6, -0.4, -0.2
            ])
        );
    }

    #[test]
    fn create_sine_ps_no_offset() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let snake = ThreeDimensionalSnake::naive_init(&config);
        let planes = config.planes as usize;
        let sine = snake.create_planes_snake_imagespace(planes);
        let sine_ps = snake.create_planes_snake_ps(&sine, 1000, 0);
        assert_eq!(
            sine_ps,
            DVector::from_vec(vec![
                0, 32, 65, 102, 147, 250, 352, 397, 434, 467, 500, 532, 565, 602, 647, 749, 852,
                897, 934, 967, 1000
            ])
        );
    }

    #[test]
    fn create_sine_ps_with_offset() {
        let config = setup_image_scanning_config().with_planes(10).build();
        let snake = ThreeDimensionalSnake::naive_init(&config);
        let planes = config.planes as usize;
        let sine = snake.create_planes_snake_imagespace(planes);
        let sine_ps = snake.create_planes_snake_ps(&sine, 1000, 10);
        assert_eq!(
            sine_ps,
            DVector::from_vec(vec![
                10, 42, 75, 112, 157, 260, 362, 407, 444, 477, 510, 542, 575, 612, 657, 759, 862,
                907, 944, 977, 1010
            ])
        );
    }
}
