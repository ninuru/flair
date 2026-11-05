use crate::terminal::TermEvent;
use alacritty_terminal::{
    event::{Event, EventListener},
    vte::ansi::Rgb,
};
use flair_core::{proto::Color, theme::Theme};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
pub(crate) struct Collector {
    pub events: Rc<RefCell<Vec<TermEvent>>>,
    pub palette: Rc<RefCell<Theme>>,
}
impl EventListener for Collector {
    fn send_event(&self, event: Event) {
        let event = match event {
            Event::Title(title) => Some(TermEvent::Title(title)),
            Event::ResetTitle => Some(TermEvent::Title(String::new())),
            Event::Bell => Some(TermEvent::Bell),
            Event::PtyWrite(text) => Some(TermEvent::PtyWrite(text.into_bytes())),
            Event::ColorRequest(index, format) => {
                let color = match index {
                    256 => Color::Foreground,
                    257 => Color::Background,
                    258 => Color::Cursor,
                    i => Color::Indexed(i.min(255) as u8),
                };
                let c = self.palette.borrow().resolve(color);
                Some(TermEvent::PtyWrite(
                    format(Rgb {
                        r: c.r,
                        g: c.g,
                        b: c.b,
                    })
                    .into_bytes(),
                ))
            }
            Event::ClipboardLoad(_, _)
            | Event::ClipboardStore(_, _)
            | Event::TextAreaSizeRequest(_)
            | Event::MouseCursorDirty
            | Event::CursorBlinkingChange
            | Event::Wakeup
            | Event::Exit
            | Event::ChildExit(_) => None,
        };
        if let Some(event) = event {
            self.events.borrow_mut().push(event);
        }
    }
}
