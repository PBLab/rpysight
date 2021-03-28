use std::ops::{Deref, Index};

use kiss3d::nalgebra::{DVector, Point3};

use crate::point_cloud_renderer::ImageCoor;

type Picosecond = i64;

/// Picosecond and Hz aware period
#[derive(Clone, Copy, Debug)]
pub struct Period {
    pub period: Picosecond,
}

impl Period {
    /// Convert a Hz-based frequency into units of picoseconds
    pub(crate) fn from_freq<T: Into<f64>>(hz: T) -> Period {
        let hz = hz.into();
        Period {
            period: ((1.0 / hz) * 1e12).round() as Picosecond,
        }
    }
}

impl Deref for Period {
    type Target = Picosecond;

    fn deref(&self) -> &Picosecond {
        &self.period
    }
}

/// Determines whether the scan was bidirectional or unidirectional
#[derive(Debug, Clone, Copy)]
pub(crate) enum Bidirectionality {
    Bidir,
    Unidir,
}

/// Current state of the app and renderer.
pub struct Context {
    last_line: Picosecond,
    last_line_image_coor: f32,
    last_frame: Picosecond,
    typical_frame_period: i64,
}

impl Context {
    pub(crate) fn new() -> Self {
        Self {
            last_line: 0,
            last_line_image_coor: 0.0,
            last_frame: 0,
            typical_frame_period: 0,
        }
    }

    pub(crate) fn set_last_line(&mut self, last_line: Picosecond) -> Option<ImageCoor> {
        self.last_line = last_line;
        self.last_line_image_coor =
            ((self.last_frame - last_line) / self.typical_frame_period) as f32;
        None
    }

    pub(crate) fn set_last_frame(&mut self, last_frame: Picosecond) -> Option<ImageCoor> {
        self.last_frame = last_frame;
        None
    }
}

/// Enumerates all possible data streams that can be handled by RPySight, like
/// PMT data, line sync events and so on.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DataType {
    Pmt1,
    Pmt2,
    Pmt3,
    Pmt4,
    Frame,
    Line,
    TagLens,
    Laser,
    Invalid,
}

const MAX_TIMETAGGER_INPUTS: usize = 18;

/// A data structure which maps the input channel to the data type it relays.
///
/// The underlying storage is a Vec, and due to the way the Index trait is
/// implemented here we can index into an Inputs instance with a positive or
/// negative value without any difference.
#[derive(Clone, Debug)]
pub(crate) struct Inputs(Vec<DataType>);

impl Inputs {
    /// Generates a new Inputs instance. Panics if the input channels aren't
    /// unique or if a channel was accidently assigned to a non-existent input.
    pub(crate) fn from_config(config: &AppConfig) -> Inputs {
        let mut data: Vec<DataType> = Vec::with_capacity(MAX_TIMETAGGER_INPUTS);
        for _ in 0..MAX_TIMETAGGER_INPUTS {
            data.push(DataType::Invalid);
        }
        let mut set = std::collections::HashSet::<usize>::new();
        let mut used_channels = 0;
        for (ch, dt) in vec![
            config.pmt1_ch.abs() as usize,
            config.pmt2_ch.abs() as usize,
            config.pmt3_ch.abs() as usize,
            config.pmt4_ch.abs() as usize,
            config.frame_ch.abs() as usize,
            config.line_ch.abs() as usize,
            config.taglens_ch.abs() as usize,
            config.laser_ch.abs() as usize,
        ]
        .into_iter()
        .zip(
            vec![
                DataType::Pmt1,
                DataType::Pmt2,
                DataType::Pmt3,
                DataType::Pmt4,
                DataType::Frame,
                DataType::Line,
                DataType::TagLens,
                DataType::Laser,
            ]
            .into_iter(),
        ) {
            if ch != 0 {
                set.insert(ch);
                data[ch] = dt;
                used_channels += 1;
            }
        }
        assert_eq!(set.len(), used_channels, "One of the channels was a duplicate");
        Inputs(data)
    }
}

impl Index<i32> for Inputs {
    type Output = DataType;

    fn index(&self, channel: i32) -> &Self::Output {
        &self.0[channel.abs() as usize]
    }
}

/// Configs
#[derive(Debug, Clone)]
pub(crate) struct AppConfig {
    pub point_color: Point3<f32>,
    rows: u32,
    columns: u32,
    planes: u32,
    scan_period: Period,
    tag_period: Period,
    bidir: Bidirectionality,
    fill_fraction: f32, // (0..100)
    frame_dead_time: Picosecond,
    pmt1_ch: i32,
    pmt2_ch: i32,
    pmt3_ch: i32,
    pmt4_ch: i32,
    laser_ch: i32,
    frame_ch: i32,
    line_ch: i32,
    taglens_ch: i32,
}

impl AppConfig {
    pub(crate) fn new(
        point_color: Point3<f32>,
        rows: u32,
        columns: u32,
        planes: u32,
        scan_period: Period,
        tag_period: Period,
        bidir: Bidirectionality,
        fill_fraction: f32,
        frame_dead_time: Picosecond,
        pmt1_ch: i32,
        pmt2_ch: i32,
        pmt3_ch: i32,
        pmt4_ch: i32,
        laser_ch: i32,
        frame_ch: i32,
        line_ch: i32,
        taglens_ch: i32,
    ) -> Self {
        AppConfig {
            point_color,
            rows,
            columns,
            planes,
            scan_period,
            tag_period,
            bidir,
            fill_fraction,
            frame_dead_time,
            pmt1_ch,
            pmt2_ch,
            pmt3_ch,
            pmt4_ch,
            laser_ch,
            frame_ch,
            line_ch,
            taglens_ch,
        }
    }
}

#[derive(Clone)]
pub(crate) struct AppConfigBuilder {
    point_color: Point3<f32>,
    rows: u32,
    columns: u32,
    planes: u32,
    scan_period: Period,
    tag_period: Period,
    bidir: Bidirectionality,
    fill_fraction: f32, // (0..100)
    frame_dead_time: Picosecond,
    pmt1_ch: i32,
    pmt2_ch: i32,
    pmt3_ch: i32,
    pmt4_ch: i32,
    laser_ch: i32,
    frame_ch: i32,
    line_ch: i32,
    taglens_ch: i32,
}

impl AppConfigBuilder {
    /// Generate an instance with default values. Useful mainly for quick
    /// testing.
    pub(crate) fn default() -> AppConfigBuilder {
        AppConfigBuilder {
            point_color: Point3::new(1.0f32, 1.0, 1.0),
            rows: 256,
            columns: 256,
            planes: 10,
            scan_period: Period::from_freq(7923.0),
            tag_period: Period::from_freq(189800.0),
            bidir: Bidirectionality::Bidir,
            fill_fraction: 71.0,
            frame_dead_time: 1_310_000_000,
            pmt1_ch: -1,
            pmt2_ch: 0,
            pmt3_ch: 0,
            pmt4_ch: 0,
            laser_ch: 0,
            frame_ch: 0,
            line_ch: 2,
            taglens_ch: 3,
        }
    }

    pub(crate) fn build(&self) -> AppConfig {
        AppConfig {
            point_color: self.point_color,
            rows: self.rows,
            columns: self.columns,
            planes: self.planes,
            scan_period: self.scan_period,
            tag_period: self.tag_period,
            bidir: self.bidir,
            fill_fraction: self.fill_fraction,
            frame_dead_time: self.frame_dead_time,
            pmt1_ch: self.pmt1_ch,
            pmt2_ch: self.pmt2_ch,
            pmt3_ch: self.pmt3_ch,
            pmt4_ch: self.pmt4_ch,
            laser_ch: self.laser_ch,
            frame_ch: self.frame_ch,
            line_ch: self.line_ch,
            taglens_ch: self.taglens_ch,
        }
    }

    pub(crate) fn with_point_color(&mut self, point_color: Point3<f32>) -> &mut Self {
        self.point_color = point_color;
        self
    }

    pub(crate) fn with_rows(&mut self, rows: u32) -> &mut Self {
        assert!(rows < 100_000);
        self.rows = rows;
        self
    }

    pub(crate) fn with_columns(&mut self, columns: u32) -> &mut Self {
        assert!(columns < 100_000);
        self.columns = columns;
        self
    }

    pub(crate) fn with_planes(&mut self, planes: u32) -> &mut Self {
        assert!(planes < 100_000);
        self.planes = planes;
        self
    }

    pub(crate) fn with_scan_period(&mut self, scan_period: Period) -> &mut Self {
        self.scan_period = scan_period;
        self
    }

    pub(crate) fn with_tag_period(&mut self, tag_period: Period) -> &mut Self {
        assert!(*tag_period > 1_000_000);
        self.tag_period = tag_period;
        self
    }

    pub(crate) fn with_bidir(&mut self, bidir: Bidirectionality) -> &mut Self {
        self.bidir = bidir;
        self
    }

    pub(crate) fn with_fill_fraction<T: Into<f32>>(&mut self, fill_fraction: T) -> &mut Self {
        let fill_fraction = fill_fraction.into();
        assert!(fill_fraction >= 0.0 && fill_fraction <= 100.0);
        self.fill_fraction = fill_fraction;
        self
    }

    pub(crate) fn with_frame_dead_time(&mut self, frame_dead_time: Picosecond) -> &mut Self {
        assert!(frame_dead_time >= 0 && frame_dead_time <= 10_000_000_000_000);
        self.frame_dead_time = frame_dead_time;
        self
    }

    pub(crate) fn with_pmt1_ch(&mut self, pmt1_ch: i32) -> &mut Self {
        assert!(pmt1_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.pmt1_ch = pmt1_ch;
        self
    }

    pub(crate) fn with_pmt2_ch(&mut self, pmt2_ch: i32) -> &mut Self {
        assert!(pmt2_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.pmt2_ch = pmt2_ch;
        self
    }

    pub(crate) fn with_pmt3_ch(&mut self, pmt3_ch: i32) -> &mut Self {
        assert!(pmt3_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.pmt3_ch = pmt3_ch;
        self
    }

    pub(crate) fn with_pmt4_ch(&mut self, pmt4_ch: i32) -> &mut Self {
        assert!(pmt4_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.pmt4_ch = pmt4_ch;
        self
    }

    pub(crate) fn with_laser_ch(&mut self, laser_ch: i32) -> &mut Self {
        assert!(laser_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.laser_ch = laser_ch;
        self
    }

    pub(crate) fn with_frame_ch(&mut self, frame_ch: i32) -> &mut Self {
        assert!(frame_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.frame_ch = frame_ch;
        self
    }

    pub(crate) fn with_line_ch(&mut self, line_ch: i32) -> &mut Self {
        assert!(line_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.line_ch = line_ch;
        self
    }

    pub(crate) fn with_taglens_ch(&mut self, taglens_ch: i32) -> &mut Self {
        assert!(taglens_ch.abs() <= MAX_TIMETAGGER_INPUTS as i32);
        self.taglens_ch = taglens_ch;
        self
    }
}

/// Marker trait to allow specific types to be used as deltas between pixels -
/// for the image space rendering case the deltas are in f32, while for the
/// rendering the deltas are in Picoseconds.
trait ImageDelta {}

impl ImageDelta for f32 {}
impl ImageDelta for Picosecond {}

/// Data regarding the step size, either in image space or in picoseconds, that
/// is needed to construct the 'snake' data vector of [`TimeToCoord`].
#[derive(Clone, Copy, Debug, PartialEq)]
struct VoxelDelta<T: ImageDelta> {
    column: T,
    row: T,
    plane: T,
    frame: T,
}

impl VoxelDelta<f32> {
    pub(crate) fn from_config(config: &AppConfig) -> VoxelDelta<f32> {
        let jump_between_columns = 1.0f32 / (config.columns as f32 - 1.0);
        let jump_between_rows = 1.0f32 / (config.rows as f32 - 1.0);
        let jump_between_planes = 1.0f32 / (config.planes as f32 - 1.0);

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
}

/// The mapping\pairing between a time in ps since the start of the experiment
/// and the image-space coordinate that this time corresponds to for the 
/// current rendered volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimeCoordPair {
    pub(crate) end_time: Picosecond,
    pub(crate) coord: ImageCoor,
}

impl TimeCoordPair {
    pub(crate) fn new(end_time: Picosecond, coord: ImageCoor) -> TimeCoordPair {
        TimeCoordPair { end_time, coord }
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
///
/// # Fields
///
/// * data: A vector of end times with their corresponding image-space
/// coordinates.
/// * last_accessed_idx: The index in data that was last used to retrieve a
/// coordinates. We keep it to look for the next matching end time only from
/// that value onward.
/// * max_frame_time: The end time for the frame. Useful to quickly check
/// whether a time tag belongs in the next frame.
/// * next_frame_starts_at: Starting time of the next frame, including the dead
/// time between frames. An offset, if you will.
/// * voxel_delta_ps: Deltas in ps of consecutive pixels, lines, etc.
/// * voxel_delta_im: Deltas in image space of consecutive pixels, lines, etc.
#[derive(Debug)]
pub(crate) struct TimeToCoord {
    data: Vec<TimeCoordPair>,
    last_accessed_idx: usize,
    last_taglens_time: Picosecond,
    max_frame_time: Picosecond,
    next_frame_starts_at: Picosecond,
    voxel_delta_ps: VoxelDelta<Picosecond>,
    voxel_delta_im: VoxelDelta<f32>,
}

impl TimeToCoord {
    /// Initialize the time -> coordinate mapping assuming that we're starting
    /// the imaging at time `offset` of the experiment.
    pub(crate) fn from_acq_params(config: &AppConfig, offset: Picosecond) -> TimeToCoord {
        let voxel_delta_ps = VoxelDelta::<Picosecond>::from_config(&config);
        let voxel_delta_im = VoxelDelta::<f32>::from_config(&config);
        if config.planes == 1 {
            let (snake, mut column_deltas_ps, column_deltas_imagespace) =
                TimeToCoord::prep_snake_2d_metadata(&config, &voxel_delta_ps, &voxel_delta_im, offset);
            TimeToCoord::generate_snake_2d_from_metadata(
                &config,
                &voxel_delta_ps,
                &voxel_delta_im,
                snake,
                &mut column_deltas_ps,
                &column_deltas_imagespace,
                offset,
            )
        } else {
            TimeToCoord::generate_snake_3d(&config, &voxel_delta_ps, &voxel_delta_im)
        }
    }

    /// Aggregate and calculate metadata for generating the 1D vector of event
    /// arrival times.
    ///
    /// To make such a snake we have to get the correct image dimensions and
    /// then populate the 'subsnakes' that will be used as a basis for the
    /// final snake.
    fn prep_snake_2d_metadata(
        config: &AppConfig,
        voxel_delta_ps: &VoxelDelta<Picosecond>,
        voxel_delta_im: &VoxelDelta<f32>,
        offset: Picosecond
    ) -> (Vec<TimeCoordPair>, DVector<Picosecond>, DVector<f32>) {
        // We add to the naive capacity 1 due to the cell containing all events
        // arriving in between frames. The number of columns for the capacity 
        // calculation includes a fake column containing the photons arriving
        // during mirror rotation. Their coordinate will contain a NaN value,
        // which means that it will not be rendered.
        let capacity = (1 + (config.rows * (config.columns + 1))) as usize;
        let snake: Vec<TimeCoordPair> = Vec::with_capacity(capacity);
        let column_deltas_ps =
            DVector::<Picosecond>::from_fn(config.columns as usize, |i, _| {
                (i as Picosecond) * voxel_delta_ps.column + voxel_delta_ps.column + offset
            });
        // Manually add the cell corresponding to events arriving during mirror
        // rotation
        let end_of_rotation_value = *&column_deltas_ps[(config.columns - 1) as usize] + voxel_delta_ps.row;
        let column_deltas_ps = column_deltas_ps.insert_rows(config.columns as usize, 1, end_of_rotation_value);
        let column_deltas_imagespace = DVector::<f32>::from_fn(config.columns as usize, |i, _| {
            (i as f32) * voxel_delta_im.column
        });
        // The events during mirror rotation will be discarded - The NaN takes
        // care of that
        let column_deltas_imagespace = column_deltas_imagespace.insert_rows(config.columns as usize, 1, f32::NAN);
        (snake, column_deltas_ps, column_deltas_imagespace)
    }

    /// Constructs the 1D vector mapping the time of arrival to image-space
    /// coordinates.
    ///
    /// This 2D vector is essentially identical to a flattened version of all
    /// pixels of the image, with two main differences: The first, it takes
    /// into account the bidirectionality of the scanner, i.e. odd rows are
    /// 'concatenated' in reverse. The second, per frame it has an extra "row"
    /// and two extra columns that should contain photons arriving between
    /// frames and while the scanner was rotating, respectively.
    ///
    /// What this function does is traverse all cells of the vector and
    /// populate them with the mapping ps -> coordinate. It's also aware of the
    /// two side columns in each row which are 'garbage'.
    fn generate_snake_2d_from_metadata(
        config: &AppConfig,
        voxel_delta_ps: &VoxelDelta<Picosecond>,
        voxel_delta_im: &VoxelDelta<f32>,
        mut snake: Vec<TimeCoordPair>,
        column_deltas_ps: &mut DVector<Picosecond>,
        column_deltas_imagespace: &DVector<f32>,
        offset: Picosecond,
    ) -> TimeToCoord {
        // Add the cell capturing all photons arriving between frames
        snake.push(TimeCoordPair::new(offset, ImageCoor::new(f32::NAN, f32::NAN, f32::NAN)));
        for row in 0..config.rows {
            let row_coord = (row as f32) * voxel_delta_im.row;
            if row == 2 { break }
            if row % 2 == 0 {
                for (column_delta_im, column_delta_ps) in column_deltas_imagespace
                    .into_iter()
                    .zip(column_deltas_ps.into_iter())
                {
                    let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, 0.5);
                    snake.push(TimeCoordPair::new(*column_delta_ps, cur_imcoor));
                }
            } else {
                for (column_delta_im, column_delta_ps) in column_deltas_imagespace.rows(0, column_deltas_imagespace.len() - 1)
                    .into_iter()
                    .rev()
                    .zip(column_deltas_ps.rows(0, column_deltas_ps.len() - 1).into_iter())
                {
                    let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, 0.5);
                    snake.push(dbg!(TimeCoordPair::new(*column_delta_ps, cur_imcoor)));
                }
                let end_of_rotation_coord = ImageCoor::new(row_coord, column_deltas_imagespace[column_deltas_imagespace.len() - 1], 0.5);
                snake.push(TimeCoordPair::new(*&column_deltas_ps[column_deltas_ps.len() - 1], end_of_rotation_coord));
            }
            let line_end = DVector::<Picosecond>::repeat(
                config.columns as usize + 1,
                column_deltas_ps[config.columns as usize],
            );
            *column_deltas_ps += &line_end;
        }
        let max_frame_time = *&snake[snake.len() - 1].end_time;
        TimeToCoord {
            data: snake,
            last_accessed_idx: 0,
            last_taglens_time: 0,
            max_frame_time,
            next_frame_starts_at: max_frame_time + voxel_delta_ps.frame,
            voxel_delta_ps: voxel_delta_ps.clone(),
            voxel_delta_im: voxel_delta_im.clone(),
        }
    }

    fn generate_snake_3d(
        config: &AppConfig,
        voxel_delta_ps: &VoxelDelta<Picosecond>,
        voxel_delta_im: &VoxelDelta<f32>,
    ) -> TimeToCoord {
        todo!()
    }

    /// The ending time, in ps, of the current volume.
    ///
    /// This function takes into account the volume size and tries to find the
    /// maximal time in ps that this frame will be active. Note that the number
    /// of planes doesn't affect this calculation because the Z scanning isn't
    /// synced to the frame buffer.
    fn calculate_max_possible_time(config: &AppConfig) -> Picosecond {
        match config.bidir {
            Bidirectionality::Bidir => Picosecond::from(config.rows) * (*config.scan_period / 2),
            Bidirectionality::Unidir => Picosecond::from(config.rows) * *config.scan_period,
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
    pub(crate) fn tag_to_coord_linear(&mut self, time: i64) -> Option<ImageCoor> {
        if time > self.max_frame_time {
            self.update_2d_data_for_next_frame();
            return self.tag_to_coord_linear(time)
        }
        let mut last_pixel_time = self.data[self.last_accessed_idx].end_time;
        let mut additional_steps_taken = 0usize;
        let mut coord = None;
        for pair in &self.data[self.last_accessed_idx..] {
            if time <= pair.end_time {
                self.last_accessed_idx += additional_steps_taken;
                coord = Some(pair.coord);
                break
            }
            additional_steps_taken += 1;
        };
        // Makes sure that we indeed captured some cell. This can be avoided in
        // principle but I'm still not confident enough in this implementation.
        if coord.is_some() {
            coord
        } else {
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
    fn update_2d_data_for_next_frame(&mut self) {
        self.last_accessed_idx = 0;
        for pair in self.data.iter_mut() {
            pair.end_time += self.next_frame_starts_at;
        }
        self.max_frame_time = self.data[self.data.len() -1].end_time;
        self.next_frame_starts_at = self.max_frame_time + self.voxel_delta_ps.frame;
        self.last_taglens_time = 0;
    }

    /// Handles a new line event
    pub(crate) fn new_line(&self, time: i64) -> Option<ImageCoor> {
        None
    }

    /// Handles a new TAG lens start-of-cycle event
    pub(crate) fn new_taglens_period(&self, time: i64) -> Option<ImageCoor> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .clone()
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
        assert_eq!(vd.row, 0.5);
        assert_eq!(vd.column, 0.25);
        assert_eq!(vd.plane, 1.0);
    }

    #[test]
    fn test_convert_fillfrac_unidir_to_deadtime() {
        let config = setup_default_config()
            .with_bidir(Bidirectionality::Unidir)
            .build();
        assert_eq!(VoxelDelta::calc_time_between_rows(&config), 99_291_327);
    }

    #[test]
    fn time_to_coord_max_possible_time_single_pixel() {
        let config = setup_default_config()
            .with_rows(1)
            .with_columns(1)
            .with_planes(1)
            .build();
        assert_eq!(
            TimeToCoord::calculate_max_possible_time(&config),
            63_082_169
        );
    }

    /// A standard frame's time is the inverse of the typical frame rate minus
    /// the time it takes the Y galvo to return back home
    #[test]
    fn time_to_coord_max_possible_time_default() {
        let config = setup_default_config().build();
        assert_eq!(
            TimeToCoord::calculate_max_possible_time(&config),
            16_149_035_264
        );
    }

    #[test]
    fn time_to_coord_snake_2d() {
        let config = setup_image_scanning_config()
            .build();
        let snake = TimeToCoord::from_acq_params(&config, 0);
        assert_eq!(
            snake.data[1],
            TimeCoordPair::new(25, ImageCoor::new(0.0, 0.0, 0.5))
        );
        assert_eq!(
            snake.data[12],
            TimeCoordPair::new(525, ImageCoor::new(1.0/9.0f32, 1.0, 0.5))
        );
    }

    #[test]
    fn time_to_coord_snake_2d_first_item_has_offset() {
        let config = setup_image_scanning_config()
            .build();
        let offset = 100;
        let snake = TimeToCoord::from_acq_params(&config, offset);
        assert_eq!(
            snake.data[0].end_time, offset);
    }

    #[test]
    fn inputs_indexing_positive() {
        let config = setup_default_config()
            .with_pmt1_ch(1)
            .with_pmt2_ch(2)
            .with_pmt3_ch(3)
            .with_pmt4_ch(4)
            .with_laser_ch(5)
            .with_frame_ch(6)
            .with_line_ch(7)
            .with_taglens_ch(8)
            .build();
        let inputs = Inputs::from_config(&config);
        assert_eq!(inputs[1], DataType::Pmt1);
    }

    #[test]
    fn inputs_indexing_negative() {
        let config = setup_default_config()
            .with_pmt1_ch(-1)
            .with_pmt2_ch(2)
            .with_pmt3_ch(3)
            .with_pmt4_ch(4)
            .with_laser_ch(5)
            .with_frame_ch(6)
            .with_line_ch(7)
            .with_taglens_ch(8)
            .build();
        let inputs = Inputs::from_config(&config);
        assert_eq!(inputs[1], DataType::Pmt1);
    }

    #[test]
    #[should_panic(expected = "One of the channels was a duplicate")]
    fn inputs_duplicate_channel() {
        let config = setup_default_config()
            .with_pmt1_ch(-1)
            .with_pmt2_ch(1)
            .build();
        let _ = Inputs::from_config(&config);
    }

    #[test]
    fn inputs_not_all_channels_are_used() {
        let config = setup_default_config()
            .with_pmt1_ch(-1)
            .with_pmt2_ch(2)
            .with_pmt3_ch(3)
            .with_pmt4_ch(4)
            .with_laser_ch(0)
            .with_frame_ch(0)
            .with_line_ch(0)
            .with_taglens_ch(0)
            .build();
        let _ = Inputs::from_config(&config);
    }
}
