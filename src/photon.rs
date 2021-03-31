use crate::rendering_helpers::Bidirectionality;

pub(crate) type ArrivalTime = u64;
pub(crate) type Coordinate = usize;
pub(crate) type Lifetime = f32;

pub(crate) enum YScanDirection {
    LeftToRight,
    RightToLeft,
}

pub(crate) enum ZScanDirection {
    TopToBottom,
    BottomToTop,
}

pub(crate) struct Photon {
    arrival_time: ArrivalTime,
    row: Option<Coordinate>,
    column: Option<Coordinate>,
    plane: Option<Coordinate>,
    lifetime: Option<Lifetime>,
}

impl Photon {
    pub fn from_arrival_time(arrival_time: ArrivalTime) -> Self {
        Self {
            arrival_time,
            row: None,
            column: None,
            plane: None,
            lifetime: None,
        }
    }
}

// A stateful pointer to the current pixel in the rendered volume.
//
// The idea is to have a pointer that is always aware of the pixel\voxel that
// the next photon in line should be placed in. We also keep track of the
// scanning direction (important for Bidirectional scanning).
pub(crate) struct BinPointer {
    pub(crate) row: i32,
    pub(crate) column: i32,
    pub(crate) plane: i32,
    ydirection: YScanDirection,
    zdirection: ZScanDirection,
}

impl BinPointer {
    pub(crate) fn new() -> Self {
        Self {
            row: 0,
            column: 0,
            plane: 0,
            ydirection: YScanDirection::LeftToRight,
            zdirection: ZScanDirection::TopToBottom,
        }
    }

    #[inline]
    fn reverse_yscan_direction(&mut self) {
        self.ydirection = match self.ydirection {
            YScanDirection::LeftToRight => YScanDirection::RightToLeft,
            YScanDirection::RightToLeft => YScanDirection::LeftToRight,
        };
    }

    #[inline]
    fn reverse_zscan_direction(&mut self) {
        self.zdirection = match self.zdirection {
            ZScanDirection::TopToBottom => ZScanDirection::BottomToTop,
            ZScanDirection::BottomToTop => ZScanDirection::TopToBottom,
        };
    }

    #[inline]
    pub(crate) fn next_row(&mut self, bidir: Bidirectionality) {
        self.row += 1;
        match bidir {
            Bidirectionality::Bidir => self.reverse_yscan_direction(),
            Bidirectionality::Unidir => self.column = 0,
        }
    }

    #[inline]
    pub(crate) fn next_column(&mut self) {
        match self.ydirection {
            YScanDirection::LeftToRight => self.column += 1,
            YScanDirection::RightToLeft => self.column -= 1,
        }
    }

    #[inline]
    pub(crate) fn next_plane(&mut self) {
        match self.zdirection {
            ZScanDirection::TopToBottom => self.plane += 1,
            ZScanDirection::BottomToTop => self.plane -= 1,
        }
    }

    #[inline]
    pub(crate) fn new_volume(&mut self) {
        self.row = 0;
        self.column = 0;
        self.plane = 0;
        self.ydirection = YScanDirection::LeftToRight;
    }
}
