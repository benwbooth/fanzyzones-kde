use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zone {
    pub id: usize,
    pub name: String,
    /// Normalized top-left-origin rectangle, matching the macOS FanzyZones model.
    /// Values are expected to be in the 0.0..1.0 range.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub applications: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub is_built_in: bool,
    #[serde(default)]
    pub padding: i32,
    pub zones: Vec<Zone>,
}

impl Layout {
    pub fn new(id: impl Into<String>, name: impl Into<String>, zones: Vec<Zone>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            is_built_in: true,
            padding: 0,
            zones,
        }
    }
}

impl Zone {
    pub fn new(
        id: usize,
        name: impl Into<String>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            x,
            y,
            width,
            height,
            applications: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn to_pixels(&self, screen: Rect, gap: i32, outer_padding: i32) -> Rect {
        let gap = gap.max(0) as f64;
        let outer_padding = outer_padding.max(0) as f64;
        let usable_x = screen.x as f64 + outer_padding;
        let usable_y = screen.y as f64 + outer_padding;
        let usable_width = (screen.width as f64 - outer_padding * 2.0).max(1.0);
        let usable_height = (screen.height as f64 - outer_padding * 2.0).max(1.0);

        let mut x = usable_x + usable_width * self.x;
        let mut y = usable_y + usable_height * self.y;
        let mut width = usable_width * self.width;
        let mut height = usable_height * self.height;

        if gap > 0.0 {
            x += gap / 2.0;
            y += gap / 2.0;
            width -= gap;
            height -= gap;
        }

        Rect {
            x: x.round() as i32,
            y: y.round() as i32,
            width: width.max(1.0).round() as i32,
            height: height.max(1.0).round() as i32,
        }
    }
}

pub fn built_in_layouts() -> Vec<Layout> {
    vec![
        Layout::new(
            "builtin.two-panes",
            "Two Panes",
            vec![
                Zone::new(0, "Left", 0.0, 0.0, 0.5, 1.0),
                Zone::new(1, "Right", 0.5, 0.0, 0.5, 1.0),
            ],
        ),
        Layout::new(
            "builtin.two-panes-wide",
            "Two Panes (Wide + Side)",
            vec![
                Zone::new(0, "Main", 0.0, 0.0, 0.7, 1.0),
                Zone::new(1, "Side", 0.7, 0.0, 0.3, 1.0),
            ],
        ),
        Layout::new(
            "builtin.three-panes",
            "Three Panes",
            vec![
                Zone::new(0, "Left", 0.0, 0.0, 1.0 / 3.0, 1.0),
                Zone::new(1, "Center", 1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
                Zone::new(2, "Right", 2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
            ],
        ),
        Layout::new(
            "builtin.three-panes-ultrawide",
            "Three Panes (Ultrawide)",
            vec![
                Zone::new(0, "Left", 0.0, 0.0, 0.25, 1.0),
                Zone::new(1, "Center", 0.25, 0.0, 0.5, 1.0),
                Zone::new(2, "Right", 0.75, 0.0, 0.25, 1.0),
            ],
        ),
        Layout::new(
            "builtin.quarters",
            "Quarters",
            vec![
                Zone::new(0, "Top-Left", 0.0, 0.0, 0.5, 0.5),
                Zone::new(1, "Top-Right", 0.5, 0.0, 0.5, 0.5),
                Zone::new(2, "Bottom-Left", 0.0, 0.5, 0.5, 0.5),
                Zone::new(3, "Bottom-Right", 0.5, 0.5, 0.5, 0.5),
            ],
        ),
        Layout::new(
            "builtin.priority-left",
            "Priority (Left Focus)",
            vec![
                Zone::new(0, "Focus", 0.0, 0.0, 0.6, 1.0),
                Zone::new(1, "Top-Right", 0.6, 0.0, 0.4, 0.5),
                Zone::new(2, "Bottom-Right", 0.6, 0.5, 0.4, 0.5),
            ],
        ),
        Layout::new(
            "builtin.grid-3x3",
            "Grid 3x3",
            (0..3)
                .flat_map(|row| {
                    (0..3).map(move |col| {
                        let id = row * 3 + col;
                        Zone::new(
                            id,
                            format!("Zone {}", id + 1),
                            col as f64 / 3.0,
                            row as f64 / 3.0,
                            1.0 / 3.0,
                            1.0 / 3.0,
                        )
                    })
                })
                .collect(),
        ),
    ]
}

pub fn clamp_layout_index(index: usize, layouts: &[Layout]) -> usize {
    if layouts.is_empty() {
        0
    } else {
        index.min(layouts.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_percent_zone_to_screen_pixels() {
        let zone = Zone::new(0, "Center", 0.25, 0.0, 0.5, 1.0);
        let rect = zone.to_pixels(
            Rect {
                x: 100,
                y: 50,
                width: 1200,
                height: 800,
            },
            0,
            0,
        );
        assert_eq!(
            rect,
            Rect {
                x: 400,
                y: 50,
                width: 600,
                height: 800
            }
        );
    }

    #[test]
    fn applies_gap_and_outer_padding() {
        let zone = Zone::new(0, "Left", 0.0, 0.0, 0.5, 1.0);
        let rect = zone.to_pixels(
            Rect {
                x: 0,
                y: 0,
                width: 1000,
                height: 500,
            },
            10,
            20,
        );
        assert_eq!(
            rect,
            Rect {
                x: 25,
                y: 25,
                width: 470,
                height: 450
            }
        );
    }

    #[test]
    fn built_in_layouts_match_expected_count() {
        assert_eq!(built_in_layouts().len(), 7);
        assert_eq!(built_in_layouts()[6].zones.len(), 9);
        assert_eq!(built_in_layouts()[1].name, "Two Panes (Wide + Side)");
        assert_eq!(built_in_layouts()[5].name, "Priority (Left Focus)");
    }
}
