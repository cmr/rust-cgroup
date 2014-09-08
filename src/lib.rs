//! CGroup management.

extern crate libc;

use std::collections::HashMap;
use std::cell::RefCell;
use std::io::{File, IoResult};

pub struct CGroup {
    /// Path to the cgroup control filesystem
    basepath: Path,
    /// Mapping from controller name to relative path from the basepath of that controller's
    /// directory
    controllers: HashMap<Vec<u8>, Path>,
}

pub struct Controller {
    path: Path,
    cache: RefCell<HashMap<Vec<u8>, Path>>,
}

/// Get the controller mappings for a process.
pub fn get_controllers(pid: libc::pid_t) -> IoResult<HashMap<Vec<u8>, Path>> {
    let contents = try!(File::open(&Path::new(format!("/proc/{}/cgroup", pid))).read_to_string());
    let mut map = HashMap::new();
    for line in contents.as_slice().lines() {
        let mut columns = line.split(':').fuse();
        match columns.next() {
            Some(_) => { },
            None => break
        }
        let name: &str = columns.next().expect("No controller name!");
        let path = Path::new(columns.next().expect("No controller path!"));
        map.insert(Vec::from_slice(name.as_bytes()), path);
    }
    Ok(map)
}

fn path_cache(path: &Path) -> IoResult<HashMap<Vec<u8>, Path>> {
    let mut map = HashMap::new();
    for path in try!(std::io::fs::readdir(path)).move_iter() {
        if !path.is_file() { break; }
        let fname = Vec::from_slice(path.filename().expect("Invalid path returned by readdir?"));
        map.insert(fname, path);
    }
    Ok(map)
}

impl CGroup {
    /// Get the CGroup for the current process.
    pub fn new() -> IoResult<CGroup> {
        CGroup::from_base_and_pid(Path::new("/sys/fs/cgroup"), unsafe { libc::getpid() })
    }

    /// Get the CGroup for a process using a given basepath
    pub fn from_base_and_pid(base: Path, pid: libc::pid_t) -> IoResult<CGroup> {
        let conts = try!(get_controllers(pid));

        Ok(CGroup {
            basepath: base,
            controllers: conts
        })
    }

    /// Get a controller from this cgroup, returning None if the named controller is not present.
    pub fn controller(&self, name: &[u8]) -> Option<Controller> {
        let mut p = self.basepath.join(name);
        match self.controllers.find_equiv(&name) {
            // remove the leading / to make the path "relative"
            Some(c) => p.push(c.path_relative_from(&Path::new("/")).expect("path_relative_from is bork?")),
            None => return None
        }
        let cache = match path_cache(&p) {
            Ok(cache) => cache,
            Err(_) => return None,
        };

        Some(Controller {
            path: p,
            cache: RefCell::new(cache),
        })
    }
}

impl Controller {
    /// Get a value for a key in this controller, None if the key doesn't exist
    pub fn get(&self, key: &[u8]) -> Option<IoResult<String>> {
        if !self.cache.borrow().contains_key_equiv(&key) {
            self.cache.borrow_mut().insert(Vec::from_slice(key), self.path.join(key));
        }

        let cache = self.cache.borrow();
        let p = cache.find_equiv(&key).expect("Cache didn't cache a key!");

        if !p.exists() && !p.is_file() {
            return None;
        }


        Some(File::open(p).read_to_string())
    }
}
