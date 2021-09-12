use crate::graph::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct Work<'a> {
    graph: &'a Graph,
    files: HashMap<FileId, bool>,
    want: HashSet<BuildId>,
    ready: HashSet<BuildId>,
}

impl<'a> Work<'a> {
    pub fn new(graph: &'a Graph) -> Self {
        Work {
            graph: graph,
            files: HashMap::new(),
            want: HashSet::new(),
            ready: HashSet::new(),
        }
    }

    fn want_build(
        &mut self,
        state: &mut State,
        last_state: &State,
        id: BuildId,
    ) -> std::io::Result<bool> {
        if self.want.contains(&id) {
            return Ok(true);
        }

        // Visit inputs first, to discover if any are out of date.
        let mut input_dirty = false;
        for &id in &self.graph.build(id).ins {
            let d = self.want_file(state, last_state, id)?;
            input_dirty = input_dirty || d;
        }

        let dirty = input_dirty
            || true /*match last_state.get_hash(id) {
                None => true,
                Some(hash) => hash != state.hash(self.graph, id)?,
            }*/;

        if dirty {
            self.want.insert(id);
            if !input_dirty {
                self.ready.insert(id);
            }
        }

        Ok(dirty)
    }

    pub fn want_file(
        &mut self,
        state: &mut State,
        last_state: &State,
        id: FileId,
    ) -> std::io::Result<bool> {
        if let Some(dirty) = self.files.get(&id) {
            return Ok(*dirty);
        }

        let dirty = match self.graph.file(id).input {
            None => {
                state.stat(self.graph, id)?;
                state.file_mut(id).hash = Some(Hash::todo()); // ready
                false
            }
            Some(bid) => {
                if self.want_build(state, last_state, bid)? {
                    true
                } else {
                    match state.stat(self.graph, id)? {
                        MTime::Missing => true,
                        MTime::Stamp(_) => {
                            // compare hash
                            false
                        }
                    }
                }
            }
        };

        self.files.insert(id, dirty);
        Ok(dirty)
    }

    fn recheck_ready(&mut self, state: &State, build: &Build) -> bool {
        println!("recheck {:?}", build.cmdline);
        build.ins.iter().all(|&id| {
            let h = state.file(id).hash.is_some();
            println!("  {:?} {}", id, h);
            h
        })
    }

    fn build_finished(&mut self, state: &mut State, id: BuildId) {
        let build = self.graph.build(id);
        println!("finished {:?}", build);
        let hash = state.hash(self.graph, id);
        for &id in &build.outs {
            let file = self.graph.file(id);
            println!("  wrote {:?} {:?}", id, file.name);
            state.file_mut(id).mtime = Some(MTime::Stamp(1));
            state.file_mut(id).hash = Some(hash);
            for &id in &file.dependents {
                if !self.want.contains(&id) {
                    continue;
                }
                if !self.recheck_ready(state, self.graph.build(id)) {
                    continue;
                }
                println!("now ready: {:?}", id);
                self.ready.insert(id);
            }
        }
    }

    pub fn run(&mut self, state: &mut State) -> std::io::Result<()> {
        while !self.want.is_empty() {
            let id = match self.ready.iter().next() {
                None => {
                    panic!("no ready, but want {:?}", self.want);
                }
                Some(&id) => id,
            };
            self.want.remove(&id);
            self.ready.remove(&id);
            let build = self.graph.build(id);
            println!("run {:?} {:?}", id, build);
            self.build_finished(state, id);
        }
        Ok(())
    }
}