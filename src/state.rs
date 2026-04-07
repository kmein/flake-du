use std::io::{Stdout, stdout};

use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eyre::Result;
use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};
use time::{UtcOffset, format_description::FormatItem};

use crate::{
    error::{IndexOutOfBounds, InvalidInput},
    lock::{Input, Lock, Resolve},
    pane::Pane,
};

pub(crate) struct State<'a> {
    current: usize,
    panes: Vec<Pane>,
    lock: Resolve,
    tz: UtcOffset,
    time_format: Vec<FormatItem<'a>>,
    term: Terminal<CrosstermBackend<Stdout>>,
}

impl<'a> State<'a> {
    pub(crate) fn new(lock: Lock, time_format: Vec<FormatItem<'a>>) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        stdout.execute(EnterAlternateScreen)?;

        let lock = lock.resolve()?;
        let tz = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let pane = Pane::new(&lock, tz, &time_format, Input::Follow(Vec::new()))?;

        Ok(Self {
            current: 0,
            panes: vec![pane],
            lock,
            tz,
            time_format,
            term: Terminal::new(CrosstermBackend::new(stdout))?,
        })
    }

    pub(crate) fn down(&mut self) -> Result<()> {
        self.select(|i| i + 1)
    }

    pub(crate) fn left(&mut self) -> Result<()> {
        if self.current != 0 {
            self.current -= 1;
            self.render()?;
        }

        Ok(())
    }

    pub(crate) fn render(&mut self) -> Result<()> {
        let pane = self
            .panes
            .get(self.current)
            .ok_or(IndexOutOfBounds(self.current))?;

        if let Some((_, cursor)) = self
            .lock
            .get(&pane.cursor)
            .ok_or_else(|| InvalidInput(pane.cursor.clone()))?
            .inputs
            .get_index(pane.selected)
        {
            match self.panes.get(self.current + 1) {
                Some(next) => {
                    if cursor != &next.cursor {
                        self.panes[self.current + 1] = self.new_pane(cursor.clone())?;
                    }
                }
                None => self.panes.push(self.new_pane(cursor.clone())?),
            }
        } else {
            self.panes.truncate(self.current + 1);
        }

        let right = self.panes.get(self.current + 1);
        let middle = &self.panes[self.current];
        let left = self.current.checked_sub(1).and_then(|idx| self.panes.get(idx));

        let mut left_state = None;
        let mut middle_state = None;
        let mut right_state = None;

        self.term.draw(|frame| {
            let rect = frame.area();
            let left_x = rect.x;
            let left_w = rect.width / 4;
            let middle_x = left_x + left_w;
            let middle_w = rect.width / 3;
            let right_x = middle_x + middle_w;
            let right_w = rect.right() - right_x;

            if let Some(left) = left {
                left_state = Some(left.render(
                    frame,
                    Rect::new(left_x, rect.y, left_w.saturating_sub(1), rect.height),
                    false,
                ));
            }
            middle_state = Some(middle.render(
                frame,
                Rect::new(middle_x, rect.y, middle_w.saturating_sub(1), rect.height),
                true,
            ));
            if let Some(right) = right {
                right_state = Some(right.render(
                    frame,
                    Rect::new(right_x, rect.y, right_w, rect.height),
                    false,
                ));
            }
        })?;

        if let Some(state) = left_state {
            self.panes[self.current - 1].state = state;
        }
        if let Some(state) = middle_state {
            self.panes[self.current].state = state;
        }
        if let Some(state) = right_state {
            self.panes[self.current + 1].state = state;
        }

        Ok(())
    }

    pub(crate) fn right(&mut self) -> Result<()> {
        if self.current + 1 < self.panes.len() {
            self.current += 1;
            self.render()?;
        }

        Ok(())
    }

    pub(crate) fn up(&mut self) -> Result<()> {
        self.select(|i| i.saturating_sub(1))
    }

    fn new_pane(&self, cursor: Input) -> Result<Pane> {
        Pane::new(&self.lock, self.tz, &self.time_format, cursor)
    }

    fn select(&mut self, f: impl Fn(usize) -> usize) -> Result<()> {
        let pane = self
            .panes
            .get_mut(self.current)
            .ok_or(IndexOutOfBounds(self.current))?;
        let new = f(pane.selected).clamp(0, pane.len.saturating_sub(1));

        if new != pane.selected {
            pane.selected = new;
            pane.state.select(Some(new));
            self.render()?;
        }

        Ok(())
    }
}

impl Drop for State<'_> {
    fn drop(&mut self) {
        let _ = self.term.backend_mut().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}
