pub(crate) type ArrivalTime = u64;
pub(crate) type Coordinate = usize;
pub(crate) type Lifetime = f32;

pub(crate) struct ImageCoor {
    x: f32,
    y: f32,
    z: Option<f32>,
}

pub(crate) enum Bidirectionality {
    Bidir,
    Unidir,
}

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
        Self { arrival_time, row: None, column: None, plane: None, lifetime: None }
    }
}

// A stateful pointer to the current pixel in the rendered volume.
//
// The idea is to have a pointer that is always aware of the pixel\voxel that
// the next photon in line should be placed in. We also keep track of the
// scanning direction (important for Bidirectional scanning).
pub(crate) struct BinPointer {
    pub(crate) row: ImageCoor,
    pub(crate) column: ImageCoor,
    pub(crate) plane: ImageCoor,
    ydirection: YScanDirection,
    zdirection: ZScanDirection,
}

impl BinPointer {
    pub(crate) fn new() -> Self {
        Self { row: 0, column: 0, plane: 0, direction: ScanDirection::LeftToRight }
    }

    #[inline]
    fn reverse_yscan_direction(self) {
        self.ydirection = match self.ydirection {
            YScanDirection::LeftToRight => YScanDirection::RightToLeft,
            YScanDirection::RightToLeft => YScanDirection::LeftToRight,
        };
    }

    #[inline]
    fn reverse_zscan_direction(self) {
        self.zdirection = match self.zdirection {
            ZScanDirection::TopToBottom => ZScanDirection::BottomToTop,
            ZScanDirection::BottomToTop => ZScanDirection::TopToBottom,
        };
    }


    #[inline]
    pub(crate) fn next_row(self, bidir: Bidirectionality) {
        self.row += 1;
        match bidir {
            Bidir => self.reverse_yscan_direction(),
            Unidir => {self.column = 0},
        }
    }

    #[inline]
    pub(crate) fn next_column(self) {
        match self.direction {
            LeftToRight => { self.column += 1 },
            RightToLeft => { self.column -= 1 },
        }
    }

    #[inline]
    pub(crate) fn next_plane(self) {
        match self.zdirection {
            TopToBottom => { self.plane += 1 },
            BottomToTop => { self.plane -= 1 },
        }
    }

    #[inline]
    pub(crate) fn new_volume(self) {
        self.row = 0;
        self.column = 0;
        self.plane = 0;
        self.ydirection = YScanDirection::LeftToRight;
    }
}

