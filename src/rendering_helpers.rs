use std::ops::Deref;

use kiss3d::nalgebra::Point3;

use crate::point_cloud_renderer::ImageCoor;

type Picosecond = i64;

/// Picosecond and Hz aware period
#[derive(Clone, Copy, Debug)]
pub struct Period {
    pub period: Picosecond
}

impl Period {
    /// Convert a Hz-based frequency into units of picoseconds
    pub(crate) fn from_freq<T: Into<f64>>(hz: T) -> Period {
        let hz = hz.into();
        Period { period: ((1.0 / hz) * 1e12).round() as Picosecond }
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
            last_line: 0, last_line_image_coor: 0.0, last_frame: 0, typical_frame_period: 0
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
    fill_fraction: f32,  // (0..100)
    frame_dead_time: Picosecond,
}

impl AppConfig {
    pub(crate) fn new(point_color: Point3<f32>, rows: u32, columns: u32, planes: u32, scan_period: Period, tag_period: Period, bidir: Bidirectionality, fill_fraction: f32, frame_dead_time: Picosecond) -> Self {
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
    fill_fraction: f32,  // (0..100)
    frame_dead_time: Picosecond,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VoxelDelta {
    column: Picosecond,
    row: Picosecond,
    plane: Picosecond,
    frame: Picosecond,
}

impl VoxelDelta {
    pub(crate) fn from_config(config: &AppConfig) -> VoxelDelta {
        let time_between_columns = VoxelDelta::calc_time_between_columns(&config);
        let time_between_rows = VoxelDelta::calc_time_between_rows(&config);
        let time_between_planes = VoxelDelta::calc_time_between_planes(&config);
        let time_between_frames = config.frame_dead_time;
        VoxelDelta { column: time_between_columns, row: time_between_rows, plane: time_between_planes, frame: time_between_frames }
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
        ((*config.scan_period / 2) as f64 * (config.fill_fraction / 100.0) as f64).round() as Picosecond
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
            Bidirectionality::Bidir => { deadtime_during_rotation },
            Bidirectionality::Unidir => { (full_time_per_line as i64) + (2 * deadtime_during_rotation) },
        }
    }
}


#[derive(Clone, Copy, Debug)]
struct EndAndCoord {
    end_time: Picosecond,
    coord: ImageCoor,
}

pub(crate) struct TimeToCoord {
    data: Vec<EndAndCoord>,
    last_idx: usize,
}

impl TimeToCoord {
    pub(crate) fn from_acq_params(config: AppConfig) -> TimeToCoord {
        let starting_point = ImageCoor::new(0.0, 0.0, 0.0);
        TimeToCoord::from_acq_params_and_start_point(config, starting_point)
    }

    pub(crate) fn from_acq_params_and_start_point(config: AppConfig, starting_point: ImageCoor) -> TimeToCoord {
        let voxel_delta = VoxelDelta::from_config(&config);
        if config.planes > 1 {
            TimeToCoord::generate_snake_2d(&config, &voxel_delta)
        } else {
            TimeToCoord::generate_snake_3d(&config, &voxel_delta)
        }
    }

    fn generate_snake_2d(config: &AppConfig, voxel_delta: &VoxelDelta) -> TimeToCoord {
        let capacity = (config.rows * config.columns) as usize;
        let max_possible_time = TimeToCoord::calculate_max_possible_time(&config);
        let row_deltas_ps = kiss3d::nalgebra::DVector::<Picosecond>::from_fn(capacity, |i, _| ((i as Picosecond) * voxel_delta.column + voxel_delta.column));
        for pixnum in 0..capacity {


        }
        todo!()
    }

    fn generate_snake_3d(config: &AppConfig, voxel_delta: &VoxelDelta) -> TimeToCoord {
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
            Bidirectionality::Bidir => { Picosecond::from(config.rows) * (*config.scan_period / 2) },
            Bidirectionality::Unidir => { Picosecond::from(config.rows) * *config.scan_period },
        }
    }

}



#[cfg(test)]
mod tests {
    use super::*;

    fn setup_default_config() -> AppConfigBuilder {
        AppConfigBuilder::default().with_point_color(Point3::new(1.0f32, 1.0, 1.0)).with_rows(256).with_columns(256).with_planes(10).with_scan_period(Period::from_freq(7926.17)).with_tag_period(Period::from_freq(189800)).with_bidir(Bidirectionality::Bidir).with_fill_fraction(71.3).with_frame_dead_time(8 * *Period::from_freq(7926.17)).clone()
    }

    #[test]
    fn test_tag_period_freq_conversion() {
        let freq = 1;  // Hz
        assert_eq!(Period::from_freq(freq).period, 1_000_000_000_000);
    }

    #[test]
    fn test_tag_period_normal_freq() {
        let freq = 189_800;  // Hz
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
        let voxel_delta = VoxelDelta { row: 18_104_579, column: 175_693, plane: 263_435, frame: 1_009_314_712 };
        assert_eq!(VoxelDelta::from_config(&config), voxel_delta)
    }

    #[test]
    fn voxel_delta_time_between_rows() {
        let config = setup_default_config().with_bidir(Bidirectionality::Bidir).build();
        assert_eq!(VoxelDelta::calc_time_between_rows(&config), 18_104_579);
    }

    #[test]
    fn test_convert_fillfrac_unidir_to_deadtime() {
        let config = setup_default_config().with_bidir(Bidirectionality::Unidir).build();
        assert_eq!(VoxelDelta::calc_time_between_rows(&config), 99_291_327);
    }

    #[test]
    fn time_to_coord_max_possible_time_single_pixel() {
        let config = setup_default_config().with_rows(1).with_columns(1).with_planes(1).build();
        assert_eq!(TimeToCoord::calculate_max_possible_time(&config), 63_082_169);
    }
    
    /// A standard frame's time is the inverse of the typical frame rate minus
    /// the time it takes the Y galvo to return back home
    #[test]
    fn time_to_coord_max_possible_time_default() {
        let config = setup_default_config().build();
        assert_eq!(TimeToCoord::calculate_max_possible_time(&config), 16_149_035_264);
    }
}
