//! Keep the initial client area inside the current monitor's real work area.
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

fn bounded_size(wanted: (u32, u32), frame: (u32, u32), work: (u32, u32)) -> (u32, u32) {
    (
        wanted.0.min(work.0.saturating_sub(frame.0 + 24).max(1)),
        wanted.1.min(work.1.saturating_sub(frame.1 + 24).max(1)),
    )
}

pub fn fit_startup(window: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let inner = window.inner_size()?;
    let outer = window.outer_size()?;
    let frame = (
        outer.width.saturating_sub(inner.width),
        outer.height.saturating_sub(inner.height),
    );
    let work = monitor.work_area();
    let (width, height) = bounded_size(
        (inner.width, inner.height),
        frame,
        (work.size.width, work.size.height),
    );
    if width != inner.width || height != inner.height {
        window.set_min_size(Some(PhysicalSize::new(
            ((860.0 * monitor.scale_factor()) as u32).min(width),
            ((420.0 * monitor.scale_factor()) as u32).min(height),
        )))?;
        window.set_size(PhysicalSize::new(width, height))?;
        window.set_position(PhysicalPosition::new(
            work.position.x + ((work.size.width.saturating_sub(width + frame.0)) / 2) as i32,
            work.position.y + ((work.size.height.saturating_sub(height + frame.1)) / 2) as i32,
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_fits_small_screens_at_supported_scales() {
        for (w, h) in [(1366u32, 768u32), (1920, 1080)] {
            for scale in [1.0, 1.25, 1.5] {
                let frame = ((2.0 * scale) as u32, (32.0 * scale) as u32);
                let work = (w, h - (48.0 * scale) as u32);
                let size = bounded_size(
                    ((1120.0 * scale) as u32, (720.0 * scale) as u32),
                    frame,
                    work,
                );
                assert!(size.0 + frame.0 <= work.0);
                assert!(size.1 + frame.1 <= work.1);
            }
        }
    }
}
