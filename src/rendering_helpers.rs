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
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
pub(crate) struct Inputs(Vec<DataType>);

impl Inputs {
    pub(crate) fn from_config(config: &AppConfig) -> Inputs {
        let mut data: Vec<DataType> = Vec::with_capacity(MAX_TIMETAGGER_INPUTS);
        for _ in 0..MAX_TIMETAGGER_INPUTS {
            data.push(DataType::Invalid);
        }
        data[config.pmt1_ch.abs() as usize] = DataType::Pmt1;
        data[config.pmt2_ch.abs() as usize] = DataType::Pmt2;
        data[config.pmt3_ch.abs() as usize] = DataType::Pmt3;
        data[config.pmt4_ch.abs() as usize] = DataType::Pmt4;
        data[config.frame_ch.abs() as usize] = DataType::Frame;
        data[config.line_ch.abs() as usize] = DataType::Line;
        data[config.taglens_ch.abs() as usize] = DataType::TagLens;
        data[config.laser_ch.abs() as usize] = DataType::Laser;
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
        assert!(*scan_period > 10_000_000);
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

    pub(crate) fn with_fill_fraction(&mut self, fill_fraction: f32) -> &mut Self {
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
/// i.e. in the range of [0, 1.0] in each dimension.
///
/// During its initialization step, a vector of time -> coordinate is built and
/// populated with the predicted timings based on the current experimental
/// configuration. This vector, referred to here as a snake, represents a 1D
/// version of the image. Its length is the total number of pixels that the
/// scanning laser will point to during each frame. For 2D images this length
/// is the total pixel count of the image, but when using a TAG lens some pixels
/// might never receive a visit in a single frame due to the resonant nature of
/// that scanning element.
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
pub(crate) struct TimeToCoord {
    data: Vec<TimeCoordPair>,
    last_taglens_time: i64,
    max_frame_time: i64,
    next_frame_starts_at: i64,
}

impl TimeToCoord {
    /// Initialize the time -> coordinate mapping assuming that we're starting
    /// the imaging at time 0 of the experiment.
    pub(crate) fn from_acq_params(config: &AppConfig) -> TimeToCoord {
        let starting_point = ImageCoor::new(0.0, 0.0, 0.0);
        let frame_start = 0;
        let voxel_delta_ps = VoxelDelta::<Picosecond>::from_config(&config);
        let voxel_delta_im = VoxelDelta::<f32>::from_config(&config);
        if config.planes == 1 {
            let (snake, mut column_deltas_ps, column_deltas_imagespace) =
                TimeToCoord::prep_snake_2d_metadata(&config, &voxel_delta_ps, &voxel_delta_im);
            TimeToCoord::generate_snake_2d_from_metadata(
                &config,
                &voxel_delta_ps,
                &voxel_delta_im,
                snake,
                &mut column_deltas_ps,
                &column_deltas_imagespace,
            )
        } else {
            TimeToCoord::generate_snake_3d(&config, &voxel_delta_ps, &voxel_delta_im)
        }
    }

    fn prep_snake_2d_metadata(
        config: &AppConfig,
        voxel_delta_ps: &VoxelDelta<Picosecond>,
        voxel_delta_im: &VoxelDelta<f32>,
    ) -> (Vec<TimeCoordPair>, DVector<Picosecond>, DVector<f32>) {
        let capacity = (config.rows * config.columns) as usize;
        let mut snake: Vec<TimeCoordPair> = Vec::with_capacity(capacity);
        let mut column_deltas_ps =
            DVector::<Picosecond>::from_fn(config.columns as usize, |i, _| {
                (i as Picosecond) * voxel_delta_ps.column + voxel_delta_ps.column
            });
        let column_deltas_imagespace = DVector::<f32>::from_fn(config.columns as usize, |i, _| {
            (i as f32) * voxel_delta_im.column
        });
        (snake, column_deltas_ps, column_deltas_imagespace)
    }

    fn generate_snake_2d_from_metadata(
        config: &AppConfig,
        voxel_delta_ps: &VoxelDelta<Picosecond>,
        voxel_delta_im: &VoxelDelta<f32>,
        mut snake: Vec<TimeCoordPair>,
        column_deltas_ps: &mut DVector<Picosecond>,
        column_deltas_imagespace: &DVector<f32>,
    ) -> TimeToCoord {
        for row in 0..config.rows {
            let row_coord = (row as f32) * voxel_delta_im.row;
            if row % 2 == 0 {
                for (column_delta_im, column_delta_ps) in column_deltas_imagespace
                    .into_iter()
                    .zip(column_deltas_ps.into_iter())
                {
                    let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, 0.5);
                    snake.push(TimeCoordPair::new(*column_delta_ps, cur_imcoor));
                }
            } else {
                for (column_delta_im, column_delta_ps) in column_deltas_imagespace
                    .into_iter()
                    .rev()
                    .zip(column_deltas_ps.into_iter())
                {
                    let cur_imcoor = ImageCoor::new(row_coord, *column_delta_im, 0.5);
                    snake.push(TimeCoordPair::new(*column_delta_ps, cur_imcoor));
                }
            }
            let line_end = DVector::<Picosecond>::repeat(
                config.columns as usize,
                voxel_delta_ps.row + column_deltas_ps[(config.columns - 1) as usize],
            );
            *column_deltas_ps += &line_end;
        }
        let max_frame_time = *&snake[snake.len() - 1].end_time;
        TimeToCoord {
            data: snake,
            last_taglens_time: 0,
            max_frame_time: max_frame_time,
            next_frame_starts_at: max_frame_time + voxel_delta_ps.frame,
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

    pub(crate) fn tag_to_coord(&self, time: i64) -> Option<ImageCoor> {
        todo!()
    }

    pub(crate) fn new_line(&self, time: i64) -> Option<ImageCoor> {
        todo!()
    }

    pub(crate) fn new_taglens_period(&self, time: i64) -> Option<ImageCoor> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let config = setup_default_config()
            .with_rows(3)
            .with_columns(3)
            .with_planes(1)
            .build();
        let snake = TimeToCoord::from_acq_params(&config);
        assert_eq!(
            snake.data[0],
            TimeCoordPair::new(14992530, ImageCoor::new(0.0, 0.0, 0.5))
        );
        assert_eq!(
            snake.data[3],
            TimeCoordPair::new(78074699, ImageCoor::new(0.5, 1.0, 0.5))
        );
    }
}
