/// An axis-aligned rectangle in frame pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub const fn right(&self) -> u32 {
        self.x + self.width
    }

    pub const fn bottom(&self) -> u32 {
        self.y + self.height
    }

    pub const fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}
