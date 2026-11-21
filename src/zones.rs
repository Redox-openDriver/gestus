use crate::Surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Corner; 4] = [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight];

    pub fn name(self) -> &'static str {
        match self {
            Corner::TopLeft => "top-left",
            Corner::TopRight => "top-right",
            Corner::BottomLeft => "bottom-left",
            Corner::BottomRight => "bottom-right",
        }
    }

    pub fn parse(text: &str) -> Option<Corner> {
        Corner::ALL.into_iter().find(|corner| corner.name() == text.trim())
    }

    pub fn contains(self, x: f32, y: f32, surface: Surface, size: f32) -> bool {
        let (left, right) = (x <= size, x >= surface.width - size);
        let (top, bottom) = (y <= size, y >= surface.height - size);
        match self {
            Corner::TopLeft => left && top,
            Corner::TopRight => right && top,
            Corner::BottomLeft => left && bottom,
            Corner::BottomRight => right && bottom,
        }
    }

    pub fn at(x: f32, y: f32, surface: Surface, size: f32) -> Option<Corner> {
        Corner::ALL.into_iter().find(|corner| corner.contains(x, y, surface, size))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    Corner(Corner),
    Quadrant(u8),
}

impl Zone {
    pub fn contains(self, x: f32, y: f32, surface: Surface, size: f32) -> bool {
        match self {
            Zone::Corner(corner) => corner.contains(x, y, surface, size),
            Zone::Quadrant(quadrant) => {
                let right = u8::from(x >= surface.width / 2.0);
                let bottom = u8::from(y >= surface.height / 2.0);
                quadrant == 1 + right + 2 * bottom
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    pub const ALL: [Edge; 4] = [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom];

    pub fn near(self, x: f32, y: f32, surface: Surface, width: f32) -> bool {
        match self {
            Edge::Left => x <= width,
            Edge::Right => x >= surface.width - width,
            Edge::Top => y <= width,
            Edge::Bottom => y >= surface.height - width,
        }
    }

    pub fn inward(self, dx: f32, dy: f32) -> (f32, f32) {
        match self {
            Edge::Left => (dx, dy.abs()),
            Edge::Right => (-dx, dy.abs()),
            Edge::Top => (dy, dx.abs()),
            Edge::Bottom => (-dy, dx.abs()),
        }
    }

    pub fn vertical(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAD: Surface = Surface { width: 117.0, height: 72.0 };

    #[test]
    fn corners_quadrants_and_edges_cover_the_expected_spots() {
        assert_eq!(Corner::at(5.0, 5.0, PAD, 12.0), Some(Corner::TopLeft));
        assert_eq!(Corner::at(110.0, 65.0, PAD, 12.0), Some(Corner::BottomRight));
        assert_eq!(Corner::at(60.0, 5.0, PAD, 12.0), None);
        assert_eq!(Corner::parse("bottom-left"), Some(Corner::BottomLeft));
        assert!(Zone::Quadrant(1).contains(10.0, 10.0, PAD, 0.0));
        assert!(Zone::Quadrant(4).contains(100.0, 60.0, PAD, 0.0));
        assert!(!Zone::Quadrant(2).contains(10.0, 60.0, PAD, 0.0));
        assert!(Edge::Right.near(114.0, 30.0, PAD, 4.0));
        assert_eq!(Edge::Right.inward(-12.0, 3.0), (12.0, 3.0));
        assert_eq!(Edge::Top.inward(2.0, 9.0), (9.0, 2.0));
    }
}
