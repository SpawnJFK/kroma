//! Undo/redo history with named states (History panel) and user snapshots.
//! States are whole-document clones; pixel buffers are shared copy-on-write, so a state only
//! costs memory for the layers that actually changed.

use crate::document::Document;

#[derive(Clone)]
pub struct HistoryState {
    pub name: String,
    pub doc: Document,
}

pub struct History {
    states: Vec<HistoryState>,
    /// Index of the current state in `states`.
    cursor: usize,
    pub limit: usize,
    pub snapshots: Vec<HistoryState>,
}

impl History {
    pub fn new(doc: &Document, name: &str) -> History {
        History {
            states: vec![HistoryState {
                name: name.into(),
                doc: doc.clone(),
            }],
            cursor: 0,
            limit: 100,
            snapshots: vec![],
        }
    }

    /// Record the document *after* an operation named `name`.
    pub fn push(&mut self, name: &str, doc: &Document) {
        self.states.truncate(self.cursor + 1);
        self.states.push(HistoryState {
            name: name.into(),
            doc: doc.clone(),
        });
        if self.states.len() > self.limit {
            let excess = self.states.len() - self.limit;
            self.states.drain(0..excess);
        }
        self.cursor = self.states.len() - 1;
    }

    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }
    pub fn can_redo(&self) -> bool {
        self.cursor + 1 < self.states.len()
    }
    pub fn undo(&mut self) -> Option<Document> {
        if self.can_undo() {
            self.cursor -= 1;
            Some(self.states[self.cursor].doc.clone())
        } else {
            None
        }
    }
    pub fn redo(&mut self) -> Option<Document> {
        if self.can_redo() {
            self.cursor += 1;
            Some(self.states[self.cursor].doc.clone())
        } else {
            None
        }
    }
    /// Jump to a state (clicking in the History panel).
    pub fn goto(&mut self, i: usize) -> Option<Document> {
        if i < self.states.len() {
            self.cursor = i;
            Some(self.states[i].doc.clone())
        } else {
            None
        }
    }
    pub fn current(&self) -> &Document {
        &self.states[self.cursor].doc
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.states.iter().map(|s| s.name.as_str())
    }
    pub fn len(&self) -> usize {
        self.states.len()
    }
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
    pub fn take_snapshot(&mut self, name: &str) {
        let doc = self.current().clone();
        self.snapshots.push(HistoryState {
            name: name.into(),
            doc,
        });
    }
    /// Restoring a snapshot is itself a history step.
    pub fn restore_snapshot(&mut self, i: usize) -> Option<Document> {
        let s = self.snapshots.get(i)?.clone();
        self.push(&format!("Snapshot: {}", s.name), &s.doc);
        Some(s.doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    #[test]
    fn undo_redo() {
        let mut d = Document::new(4, 4, Some(Color::WHITE));
        let mut h = History::new(&d, "New");
        d.layers[0].image_mut().fill(Color::BLACK);
        h.push("Fill", &d);
        let back = h.undo().unwrap();
        assert_eq!(back.layers[0].image().get(0, 0)[0], 1.0);
        let fwd = h.redo().unwrap();
        assert_eq!(fwd.layers[0].image().get(0, 0)[0], 0.0);
        h.undo();
        h.push("Other", &d);
        assert!(!h.can_redo());
    }
}
