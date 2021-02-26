// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::sync::atomic::Ordering;

pub struct ColorEscape(pub &'static str);

impl core::fmt::Display for ColorEscape {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if super::logging::USE_COLOR.load(Ordering::Relaxed) {
            write!(f, "{}", self.0)
        } else {
            Ok(())
        }
    }
}

pub const CLEAR: ColorEscape = ColorEscape("\x1B[0m");
pub const BLACK: ColorEscape = ColorEscape("\x1B[30m");
pub const RED: ColorEscape = ColorEscape("\x1B[31m");
pub const GREEN: ColorEscape = ColorEscape("\x1B[32m");
pub const YELLOW: ColorEscape = ColorEscape("\x1B[33m");
pub const BLUE: ColorEscape = ColorEscape("\x1B[34m");
pub const MAGENTA: ColorEscape = ColorEscape("\x1B[35m");
pub const CYAN: ColorEscape = ColorEscape("\x1B[36m");
pub const WHITE: ColorEscape = ColorEscape("\x1B[37m");
pub const BRIGHT_BLACK: ColorEscape = ColorEscape("\x1B[90m");
pub const BRIGHT_RED: ColorEscape = ColorEscape("\x1B[91m");
pub const BRIGHT_GREEN: ColorEscape = ColorEscape("\x1B[92m");
pub const BRIGHT_YELLOW: ColorEscape = ColorEscape("\x1B[93m");
pub const BRIGHT_BLUE: ColorEscape = ColorEscape("\x1B[94m");
pub const BRIGHT_NAGENTA: ColorEscape = ColorEscape("\x1B[95m");
pub const BRIGHT_CYAN: ColorEscape = ColorEscape("\x1B[96m");
pub const BRIGHT_WHITE: ColorEscape = ColorEscape("\x1B[97m");
