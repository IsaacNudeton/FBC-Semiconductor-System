pub mod overview;
pub mod power;
pub mod analog;
pub mod vectors;
pub mod board;
pub mod device;
pub mod terminal;
pub mod eeprom;
pub mod testplan;
pub mod firmware;
pub mod pattern;
pub mod facility;
pub mod waveform;
pub mod datalogs;

use crate::ui::Ui;
use crate::layout::Rect;
use crate::state::{AppState, View};

/// Dispatch drawing to the active sub-panel
pub fn draw_panel(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    match state.active_view {
        View::Overview  => overview::draw(ui, rect, state),
        View::Power     => power::draw(ui, rect, state),
        View::Analog    => analog::draw(ui, rect, state),
        View::Vectors   => vectors::draw(ui, rect, state),
        View::Board     => board::draw(ui, rect, state),
        View::Device    => device::draw(ui, rect, state),
        View::Terminal  => terminal::draw(ui, rect, state),
        View::Eeprom    => eeprom::draw(ui, rect, state),
        View::TestPlan  => testplan::draw(ui, rect, state),
        View::Firmware  => firmware::draw(ui, rect, state),
        View::Pattern   => pattern::draw(ui, rect, state),
        View::Facility  => facility::draw(ui, rect, state),
        View::Waveform  => waveform::draw(ui, rect, state),
        View::Datalogs  => datalogs::draw(ui, rect, state),
    }
}
