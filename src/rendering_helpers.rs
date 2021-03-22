use std::ops::Deref;

use kiss3d::nalgebra::Point3;

use crate::point_cloud_renderer::ImageCoor;

type Picosecond = i64;

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
        let time_between_pixels = (*config.scan_period / 2) / (config.columns as Picosecond);
        let time_between_rows = config.convert_fillfrac_to_deadtime();
        let time_between_frames = config.frame_dead_time;
        todo!()
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

/// Picosecond and Hz aware period
#[derive(Clone, Copy, Debug)]
pub struct Period {
    pub period: Picosecond
}

impl Period {
    /// Convert a Hz-based frequency into units of picoseconds
    pub(crate) fn from_freq(hz: Picosecond) -> Period {
        let hz = hz as f64;
        Period { period: ((1.0 / hz) * 1e12).round() as Picosecond }
    }
}

impl Deref for Period {
    type Target = Picosecond;

    fn deref(&self) -> &Picosecond {
        &self.period
    }
}

/// Configs
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
    
    /// Returns the number of picoseconds since we last were on a pixel.
    ///
    /// If the scan is bidirectional, this time corresponds to the time it
    /// the mirror to slow down, turn and accelerate back to the next line in
    /// the opposite direction. If the scan is unidirectional then the method
    /// factors in the time it takes the mirror to move to its starting
    /// position in the opposite side of the image.
    pub(crate) fn convert_fillfrac_to_deadtime(&self) -> Picosecond {
        let full_time_per_line = (*self.scan_period / 2) as f64;
        let time_per_line_after_ff = (*self.scan_period / 2) as f64 * (self.fill_fraction / 100.0) as f64;
        let deadtime_during_rotation = full_time_per_line - time_per_line_after_ff;
        match self.bidir {
            Bidirectionality::Bidir => { deadtime_during_rotation.round() as i64 },
            Bidirectionality::Unidir => { (full_time_per_line as i64) + (2 * deadtime_during_rotation.round() as i64) },
        }
    }
}

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
    pub(crate) fn default() -> AppConfigBuilder {
        AppConfigBuilder {
            point_color: Point3::new(1.0f32, 1.0, 1.0),
            rows: 256,
            columns: 256,
            planes: 10,
            scan_period: Period::from_freq(7923),
            tag_period: Period::from_freq(1898000),
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
        assert!(*tag_period > 10_000_000);
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_convert_fillfrac_bidir_to_deadtime() {
        let config = AppConfigBuilder::default().with_bidir(Bidirectionality::Bidir).build();
        assert_eq!(config.convert_fillfrac_to_deadtime(), 18_301_150);
    }

    #[test]
    fn test_convert_fillfrac_unidir_to_deadtime() {
        let config = AppConfigBuilder::default().with_bidir(Bidirectionality::Unidir).build();
        assert_eq!(config.convert_fillfrac_to_deadtime(), 99_709_709);
    }
    
}
