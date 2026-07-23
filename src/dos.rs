//! File and stream manipulation (dos.library).

use core::ffi::{CStr, c_void};
use core::fmt::Debug;

use crate::{
    Version, i32_from_usize, usize_from_i32, error::{Result, Error}
};

/// Protection bits for a file or directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ProtectionBits(pub i32);

impl ProtectionBits {
    /// Indicates the hold attribute.
    pub const HOLD: i32          = amiga_sys::FIBF_HOLD as i32;
    /// Indicates that the file is a script.
    pub const SCRIPT: i32        = amiga_sys::FIBF_SCRIPT as i32;
    /// Indicates that the file a pure command, which can be made resident.
    pub const PURE: i32          = amiga_sys::FIBF_PURE as i32;
    /// Indicates that the file has been archived.
    pub const ARCHIVE: i32       = amiga_sys::FIBF_ARCHIVE as i32;
    /// Indicates read protection: 0 = can be read, 1 = cannot be read.
    pub const READ: i32          = amiga_sys::FIBF_READ as i32;
    /// Indicates write protection: 0 = can be written, 1 = cannot be written.
    pub const WRITE: i32         = amiga_sys::FIBF_WRITE as i32;
    /// Indicates execution protection: 0 = can be executed, 1 = cannot be executed.
    pub const EXECUTE: i32       = amiga_sys::FIBF_EXECUTE as i32;
    /// Indicates delete protection: 0 = can be deleted, 1 = cannot be deleted.
    pub const DELETE: i32        = amiga_sys::FIBF_DELETE as i32;
}

/// Date and time information.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Hash, Default)]
pub struct DateStamp {
    /// Number of days since January 1, 1978.
    pub days: i32,
    /// Number of minutes past midnight.
    pub minutes: i32,
    /// Number of ticks past minute. One second has 50 ticks.
    pub ticks: i32,
}

/// DOS library object.
pub struct Dos {
    pub(crate) dos_lib: *mut amiga_sys::Library,
    initial_currentdir_lock: Option<amiga_sys::BPTR>,
}

impl Dos {
    // this is not public, because calling this new() multiple times would
    // mess up initial_currentdir_lock
    pub(crate) fn new() -> Result<Dos> {
        let dos_lib = unsafe {
            amiga_sys::OpenLibrary(amiga_sys::abs_exec_library(), b"dos.library\0".as_ptr(), 0)
        };
        if dos_lib.is_null() {
            return Err(crate::error::Error::UnsupportedLibraryVersion);
        }
        Ok(Dos {
            dos_lib,
            initial_currentdir_lock: None,
        })
    }

    /// Returns the input stream for the program.
    ///
    /// Programs launched from Workbench return `None`.
    ///
    /// This function calls the dos.library `Input` function.
    pub fn input(&mut self) -> Option<File> {
        let res = unsafe { amiga_sys::Input(self.dos_lib) };
        if res == 0 {
            return None;
        }
        Some(File {
            dos_lib: self.dos_lib,
            file: res,
            needs_closing: false,
        })
    }

    /// Returns the output stream for the program.
    ///
    /// Programs launched from Workbench return `None`.
    ///
    /// This function calls the dos.library `Output` function.
    pub fn output(&mut self) -> Option<File> {
        let res = unsafe { amiga_sys::Output(self.dos_lib) };
        if res == 0 {
            return None;
        }
        Some(File {
            dos_lib: self.dos_lib,
            file: res,
            needs_closing: false,
        })
    }

    /// Creates a directory.
    ///
    /// The directory is created and an exclusive lock is returned for it. Only the directory
    /// for the last component of the path is created. Intermediate directories are not created.
    /// Returns an error if a file or directory already exists with the same name.
    ///
    /// This function calls the dos.library `CreateDir` function.
    pub fn create_dir(&mut self, path: &CStr) -> Result<FileLock> {
        validate_path(path)?;
        let res =
            unsafe { amiga_sys::CreateDir(self.dos_lib, path.as_ptr() as *const u8) };
        if res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(FileLock {
            dos_lib: self.dos_lib,
            sys_lock: res,
            needs_closing: true,
        })
    }

    /// Renames a file or a directory.
    ///
    /// Renaming fails if a file with the `to` name already exists. It is not possible to rename
    /// a file from one volume to another.
    ///
    /// This method calls the dos.library `Rename` function.
    pub fn rename(&mut self, from: &CStr, to: &CStr) -> Result<()> {
        validate_path(from)?;
        validate_path(to)?;
        // TODO: V34 has a bug related to dir renaming into itself, check for that? how to compare
        // absolute and relative paths?
        let res = unsafe {
            amiga_sys::Rename(
                self.dos_lib,
                from.as_ptr() as *const u8,
                to.as_ptr() as *const u8,
            )
        };
        if res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(())
    }

    /// Removes a file, a directory or a link from the file system.
    ///
    /// Removing a directory will fail if it is not empty.
    ///
    /// Returns an error if removal failed.
    ///
    /// This method calls the dos.library `DeleteFile` function.
    pub fn remove(&mut self, path: &CStr) -> Result<()> {
        validate_path(path)?;
        let res = unsafe { amiga_sys::DeleteFile(self.dos_lib, path.as_ptr() as *const u8) };
        if res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(())
    }

    /// Sets a comment for a file or a directory.
    ///
    /// Some file systems may not support comments. Also, the maximum length of the comment varies.
    /// For the system provided file systems, the maximum length is 80 bytes.
    ///
    /// Returns an error if setting the comment fails.
    ///
    /// This method calls the dos.library `SetComment` function.
    pub fn set_comment(&mut self, path: &CStr, comment: &CStr) -> Result<()> {
        validate_path(path)?;
        let res = unsafe {
            amiga_sys::SetComment(
                self.dos_lib,
                path.as_ptr() as *const u8,
                comment.as_ptr() as *const u8,
            )
        };
        if res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(())
    }

    /// Changes protection bits (permissions) for a file or a directory.
    ///
    /// Returns an error if setting the protection bits fails.
    ///
    /// This method calls the dos.library `SetProtection` function.
    pub fn set_protection(&mut self, path: &CStr, bits: ProtectionBits) -> Result<()> {
        validate_path(path)?;
        let res = unsafe {
            amiga_sys::SetProtection(self.dos_lib, path.as_ptr() as *const u8, bits.0)
        };
        if res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(())
    }

    /// Changes the current directory to point to a directory or file.
    ///
    /// This method consumes the given file lock.
    /// Returns the previous current directory as a file lock.
    ///
    /// Note: it is possible to change the current directory to point to a file and
    /// open the file by calling [`File::open()`] with an empty file name.
    ///
    /// This method calls the dos.library `CurrentDir` function.
    pub fn change_current_dir(&mut self, file_lock: FileLock) -> Result<FileLock> {
        let prev_cdir_lock = unsafe {
            amiga_sys::CurrentDir(self.dos_lib, file_lock.sys_lock)
        };
        // there's no errors to detect, any lock can be made the current dir

        // if this was the first call to current_dir(), store the result (old dir) so that it
        // can restored when the DOS library is dropped
        // the initial lock is not closed by FileLock, because it is owned by the system
        let needs_closing = if self.initial_currentdir_lock.is_none() {
            self.initial_currentdir_lock = Some(prev_cdir_lock);
            false
        } else {
            true
        };
        // self.sys_lock is owned by the system after call to CurrentDir() and must not
        // be freed (dropped)
        core::mem::forget(file_lock);
        Ok(FileLock {
            dos_lib: self.dos_lib,
            sys_lock: prev_cdir_lock,
            needs_closing,
        })
    }

    /// Gets the system current date and time.
    ///
    /// This time measurement **is not monotonic**.
    ///
    /// This method calls the dos.library `DateStamp` function.
    pub fn date_stamp(&mut self) -> DateStamp {
        let mut newstamp = amiga_sys::DateStamp {
            ds_Days: 0,
            ds_Minute: 0,
            ds_Tick: 0,
        };
        // DateStamp() returns the same struct it was passed in..
        let curstamp = unsafe {
            amiga_sys::DateStamp(self.dos_lib, (&mut newstamp) as *mut amiga_sys::DateStamp)
        };
        unsafe {
            DateStamp {
                days: (*curstamp).ds_Days,
                minutes: (*curstamp).ds_Minute,
                ticks: (*curstamp).ds_Tick,
            }
        }
    }

    /// Delays the process for a number of ticks.
    ///
    /// This method blocks the process for a specified number of ticks. Zero and negative
    /// ticks return immediately. One second has 50 ticks.
    ///
    /// This method uses a timer to perform the delay. No CPU blocking busy-looping happens.
    ///
    /// This method calls the dos.library `Delay` function.
    pub fn delay(&mut self, ticks: i32) {
        if ticks <= 0 {
            return;
        }
        unsafe { amiga_sys::Delay(self.dos_lib, ticks) }
    }

    /// Returns the version of dos.library.
    pub fn version(&self) -> Version {
        unsafe {
            Version {
                version: (*self.dos_lib).lib_Version,
                revision: (*self.dos_lib).lib_Revision
            }
        }
    }

    /// Returns a pointer to the underlying system Dos Library object.
    pub fn sys_dos(&mut self) -> *mut amiga_sys::Library {
        self.dos_lib
    }
}

impl Drop for Dos {
    fn drop(&mut self) {
        // close library

        // set the initial current dir back so that shell is not confused
        // (alternatively, this could call SetCurrentDirName() to match the current dir?)
        if let Some(initial_cdir) = self.initial_currentdir_lock {
            unsafe {
                let old_lock = amiga_sys::CurrentDir(self.dos_lib, initial_cdir);
                amiga_sys::UnLock(self.dos_lib, old_lock);
            }
        }
        unsafe { amiga_sys::CloseLibrary(amiga_sys::abs_exec_library(), self.dos_lib); }
    }
}

/// Validates that path is not too long.
fn validate_path(path: &CStr) -> Result<()> {
    // TODO: what is the BCPL string max length?
    if path.count_bytes() > 254 {
        return Err(Error::PathTooLong);
    }
    // TODO: check that volume name "vvv:" is not longer than 30 chars
    Ok(())
}

/// Enumeration of possible methods to seek within an I/O object.
///
/// It is used by the [`File::seek()`] method.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SeekFrom {
    /// Seeks to the specified byte offset from the start. The value should be a positive value
    /// and smaller or equal to the file size.
    Start(u64),
    /// Seeks to the end position plus the specified byte offset. Zero or negative values should
    /// be used to avoid seeking beyond the end.
    End(i64),
    /// Seeks to the current position plus the specified byte offset.
    Current(i64),
}

/// Raw `BPTR` file handle.
pub type RawHandle = amiga_sys::BPTR;

/// File objects provide access to files and streams.
///
/// Files can be read, written and seeked.
///
/// Files are automatically closed when they go out of scope. Errors detected on closing are
/// ignored by the implementation of `Drop`.
///
/// Reading and writing are unbuffered and inefficient for small amounts of bytes.
pub struct File {
    dos_lib: *mut amiga_sys::Library,
    file: amiga_sys::BPTR,
    needs_closing: bool,
}

impl File {
    // there's no function to open a file in MODE_READWRITE, because it works inconsistently
    // in V34 and V36

    /// Opens an existing file in read-write mode.
    ///
    /// The current seek position is at the start of the file.
    ///
    /// This function calls the dos.library `Open` function.
    pub fn open(dos: &mut Dos, path: &CStr) -> Result<File> {
        validate_path(path)?;
        let res = unsafe {
            amiga_sys::Open(
                dos.dos_lib,
                path.as_ptr() as *const u8,
                amiga_sys::MODE_OLDFILE as i32,
            )
        };
        if res == 0 {
            return Err(get_ioerr(dos.dos_lib));
        }
        Ok(File {
            dos_lib: dos.dos_lib,
            file: res,
            needs_closing: true,
        })
    }

    /// Creates and opens a new file in read-write mode with an exclusive lock.
    ///
    /// This function creates a file if it does not exist and deletes existing old files.
    ///
    /// Access to the file is exclusive. Other processes can't access the file.
    ///
    /// This function calls the dos.library `Open` function.
    pub fn create(dos: &mut Dos, path: &CStr) -> Result<File> {
        validate_path(path)?;
        let res = unsafe {
            amiga_sys::Open(
                dos.dos_lib,
                path.as_ptr() as *const u8,
                amiga_sys::MODE_NEWFILE as i32,
            )
        };
        if res == 0 {
            return Err(get_ioerr(dos.dos_lib));
        }
        Ok(File {
            dos_lib: dos.dos_lib,
            file: res,
            needs_closing: true,
        })
    }

    /// Closes the file.
    ///
    /// This method consumes this file object. Returns an error if closing fails. If closing fails,
    /// the file handle is still deallocated.
    ///
    /// Systems older than Kickstart 2.0 (< V36) never report any closing errors.
    ///
    /// This method calls the dos.library `Close` function.
    pub fn close(mut self) -> Result<()> {
        if !self.needs_closing || self.file == 0 {
            return Ok(());
        }
        self.needs_closing = false;
        let res = unsafe { amiga_sys::Close(self.dos_lib, self.file) };
        // V36 is the first version to return a value from Close(), older versions don't return it!
        let dos_version = unsafe { (*self.dos_lib).lib_Version };
        if dos_version >= 36 && res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(())
    }

    /// Reads bytes to the specified buffer, returning how many bytes were read.
    ///
    /// This method may read less bytes than `buf` len. Always inspect the returned value
    /// to check how many bytes were actually read. This method may block for interactive
    /// files until there is data to be read.
    ///
    /// Returns 0 if the end of stream has been reached or if `buf` length was 0.
    /// Returns an error if reading failed.
    ///
    /// This is an unbuffered method.
    ///
    /// This method calls the dos.library `Read` function.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let res = unsafe {
            amiga_sys::Read(
                self.dos_lib,
                self.file,
                buf.as_mut_ptr() as *mut c_void,
                i32_from_usize(buf.len()),
            )
        };
        if res < 0 {
            // TODO: map IoErr() to error? to match core::io Result, return core io error?
            return Err(crate::error::Error::ReadError);
        }
        Ok(usize_from_i32(res))
    }

    /// Reads exact number of bytes to fill `buf`.
    ///
    /// Reading ends if any errors are encountered, including reaching end of file before `buf`
    /// is full. In case of errors, the contents of `buf` and how many bytes were read are
    /// unspecified.
    ///
    /// This method calls the dos.library `Read` function.
    pub fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => buf = &mut buf[n..],
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(Error::UnexpectedEof)
        } else {
            Ok(())
        }
    }

    /// Detects if characters are available to be read within the given time.
    ///
    /// This blocking method waits for characters to be available for reading. Returns
    /// immediately `false` if the file is not an interactive file.
    ///
    /// Returns `true` if characters are available to be read within `microsecs`.
    /// Otherwise, returns `false`.
    ///
    /// This method calls the dos.library `WaitForChar` function.
    pub fn wait_for_char(&mut self, microsecs: i32) -> Result<bool> {
        let mut microsecs = microsecs;
        // avoid bug in V35 and earlier
        if microsecs <= 0 {
            microsecs = 1;
        }
        let res = unsafe {
            amiga_sys::WaitForChar(self.dos_lib, self.file, microsecs)
        };
        if res == 0 {
            let ioerr = unsafe { amiga_sys::IoErr(self.dos_lib) };
            if ioerr != 0 {
                return Err(Error::IoErr(ioerr));
            }
            return Ok(false);
        }
        Ok(true)
    }

    /// Writes a buffer of bytes, returning how many bytes were written.
    ///
    /// Returns the number of bytes written, which can be even 0.
    /// Returns an error if writing failed.
    ///
    /// This is an unbuffered method and may block until the receiving file (e.g., a printer) can
    /// accept more data.
    ///
    /// This method calls the dos.library `Write` function.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let res = unsafe {
            amiga_sys::Write(
                self.dos_lib,
                self.file,
                buf.as_ptr() as *const c_void,
                i32_from_usize(buf.len()),
            )
        };
        if res < 0 {
            // TODO: map IoErr() to error? to match core::io Result, return core io error?
            return Err(crate::error::Error::WriteError);
        }
        Ok(usize_from_i32(res))
    }

    /// Writes all bytes of a buffer.
    ///
    /// This method tries to write until all bytes in `buf` have been successfully written.
    ///
    /// Returns an error if writing fails.
    ///
    /// This method calls the dos.library `Write` function.
    pub fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {}
                Ok(n) => buf = &buf[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Seeks to the specified byte offset.
    ///
    /// It is not possible to seek beyond the end of a stream.
    ///
    /// Returns the new position from the start of the stream or an error if seeking failed.
    ///
    /// This method calls the dos.library `Seek` function.
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let (sys_pos, sys_mode) = match pos {
            SeekFrom::Start(pos) => {
                let p = i32::try_from(pos)
                    .map_err(|_| crate::error::Error::SeekError)?;
                (p, amiga_sys::OFFSET_BEGINNING)
            },
            SeekFrom::End(pos) => {
                let p = i32::try_from(pos)
                    .map_err(|_| crate::error::Error::SeekError)?;
                (p, amiga_sys::OFFSET_END as i32)
            },
            SeekFrom::Current(pos) => {
                let p = i32::try_from(pos)
                    .map_err(|_| crate::error::Error::SeekError)?;
                (p, amiga_sys::OFFSET_CURRENT as i32)
            },
        };
        let res = unsafe {
            amiga_sys::Seek(self.dos_lib, self.file, sys_pos, sys_mode)
        };
        if res == -1 {
            // TODO: map IoErr() to error? to match core::io Result, return core io error?
            return Err(crate::error::Error::SeekError);
        }
        // check IoErr() because V36 and V37 returns the current position instead of -1 on an error
        let ioerr = unsafe { amiga_sys::IoErr(self.dos_lib) };
        if ioerr != 0 {
            // TODO: map IoErr() to error? to match core::io Result, return core io error?
            return Err(crate::error::Error::IoErr(ioerr));
        }
        // casts signed result (position) to unsigned on purpose for file systems which
        // support files greater than 2 gigabytes
        Ok(u64::from(res.cast_unsigned()))
    }

    /// Returns the current position from the start of the stream.
    ///
    /// This is the same as `self.seek(SeekFrom::Current(0))`.
    pub fn stream_position(&mut self) -> Result<u64> {
        self.seek(SeekFrom::Current(0))
    }

    /// Returns `true` if this file refers to an interactive file, such as a virtual terminal,
    /// serial port or parallel port.
    ///
    /// This method calls the dos.library `IsInteractive` function.
    pub fn is_interactive(&mut self) -> bool {
        unsafe {
            amiga_sys::IsInteractive(self.dos_lib, self.file) != 0
        }
    }

    /// Flushes this stream, ensuring that all intermediately buffered contents
    /// reach their destination.
    ///
    /// This method always returns `Ok`, except on Kickstart 1.3 and lower where this method
    /// is not supported.
    ///
    /// This function calls the dos.library `Flush` function.
    #[cfg(feature = "v36")]
    pub fn flush(&mut self) -> Result<()> {
        if unsafe { (*self.dos_lib).lib_Version } < 36 {
            return Err(Error::UnsupportedLibraryVersion);
        }
        unsafe { amiga_sys::Flush(self.dos_lib, self.file); }
        Ok(())
    }

    /// Extracts the raw file handle.
    ///
    /// This function is typically used to borrow an owned file handle. When used in this way,
    /// this method does not pass ownership of the raw file handle to the caller, and
    /// the file handle is only guaranteed to be valid while the original object has not yet
    /// been destroyed.
    ///
    /// This function may return 0, which means the root of the file system.
    pub fn as_raw_handle(&self) -> RawHandle {
        self.file
    }

    /// Consumes this object, returning the underlying raw file handle.
    ///
    /// It is up to the caller to close the file handle, if it needs to be closed
    /// (for instance, file handles from `input()` and `output()` must not be closed).
    ///
    /// This function may return 0, which means the root of the file system.
    pub fn into_raw_handle(self) -> RawHandle {
        let handle = self.file;
        core::mem::forget(self);
        handle
    }
}

impl Drop for File {
    fn drop(&mut self) {
        // close file
        if !self.needs_closing {
            return;
        }
        // closing zero file handle may crash older systems and is a no-op on newer systems
        if self.file == 0 {
            return;
        }
        unsafe {
            // V34 and earlier have an undefined return value..
            let _ = amiga_sys::Close(self.dos_lib, self.file);
        }
    }
}

/// Modes for FileLocks.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileLockMode {
    /// Shared access mode.
    Shared = amiga_sys::SHARED_LOCK,
    /// Exclusive access mode.
    Exclusive = amiga_sys::EXCLUSIVE_LOCK,
}

/// FileLock provides locks for files and directories.
///
/// FileLocks are automatically closed when they go out of scope.
pub struct FileLock {
    dos_lib: *mut amiga_sys::Library,
    sys_lock: amiga_sys::BPTR,
    needs_closing: bool,
}

impl Debug for FileLock {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FileLock")
            .finish()
    }
}

impl FileLock {
    /// Open a new lock for a directory or file.
    ///
    /// This function calls the dos.library `Lock` function.
    pub fn open(
        dos: &mut Dos,
        path: &CStr,
        mode: FileLockMode,
    ) -> Result<FileLock> {
        validate_path(path)?;
        let res = unsafe {
            amiga_sys::Lock(
                dos.dos_lib,
                path.as_ptr() as *const u8,
                mode as i32,
            )
        };
        if res == 0 {
            return Err(get_ioerr(dos.dos_lib));
        }
        Ok(FileLock {
            dos_lib: dos.dos_lib,
            sys_lock: res,
            needs_closing: true,
        })
    }

    pub(crate) fn from_raw_bptr(
        dos_lib: *mut amiga_sys::Library,
        bptr: amiga_sys::BPTR,
        needs_closing: bool,
    ) -> FileLock {
        FileLock {
            dos_lib,
            sys_lock: bptr,
            needs_closing,
        }
    }

    /// Creates a duplicate of a shared lock.
    ///
    /// Exclusive locks cannot be duplicated.
    ///
    /// This method calls the dos.library `DupLock` function.
    pub fn duplicate(&mut self) -> Result<FileLock> {
        let res = unsafe { amiga_sys::DupLock(self.dos_lib, self.sys_lock) };
        if res == 0 {
            return Err(get_ioerr(self.dos_lib));
        }
        Ok(FileLock {
            dos_lib: self.dos_lib,
            sys_lock: res,
            needs_closing: true,
        })
    }

    /// Obtains a shared lock for this lock's parent directory.
    ///
    /// If this file lock is the root directory of the file system ("zero lock"),
    /// then a duplicate of it is returned.
    ///
    /// This method calls the dos.library `ParentDir` function.
    pub fn parent_dir(&mut self) -> Result<FileLock> {
        let res = unsafe { amiga_sys::ParentDir(self.dos_lib, self.sys_lock) };
        if res == 0 { // this is the root lock or an error
            let ioerr = unsafe { amiga_sys::IoErr(self.dos_lib) };
            if ioerr != 0 {
                return Err(Error::IoErr(ioerr));
            }
        }
        Ok(FileLock {
            dos_lib: self.dos_lib,
            sys_lock: res,
            needs_closing: true,
        })
    }

    /// Returns an iterator to examine metadata about a file or directory referenced by this lock.
    ///
    /// The first item is metadata about the file or directory itself. The first item cannot
    /// identify hard or soft links correctly, because the link has already been followed
    /// at that point.
    ///
    /// For directories, subsequent iterator items are entries under the directory.
    pub fn examine(&mut self) -> Result<Examine> {
        // TODO: FileInfoBlock consumes quite a lot of stack, should it be allocated to heap?
        let fiba = FileInfoBlockAligner {
            sys_fib: amiga_sys::FileInfoBlock {
                fib_DiskKey: 0,
                fib_DirEntryType: 0,
                fib_FileName: [0; 108],
                fib_Protection: 0,
                fib_EntryType: 0,
                fib_Size: 0,
                fib_NumBlocks: 0,
                fib_Date: amiga_sys::DateStamp {
                    ds_Days: 0,
                    ds_Minute: 0,
                    ds_Tick: 0,
                },
                fib_Comment: [0; 80],
                fib_OwnerUID: 0,
                fib_OwnerGID: 0,
                fib_Reserved: [0; 32],
            },
        };
        Ok(Examine {
            dos_lib: self.dos_lib,
            sys_lock: self.sys_lock,
            fiba,
            first_entry: true,
            is_done: false,
        })
    }

    /// Returns true if two locks point to the same physical device.
    ///
    /// This function may return an incorrect result if it is unable to identify the underlying
    /// file systems or devices.
    ///
    /// This function calls the dos.library `SameDevice` function.
    #[cfg(feature = "v37")]
    pub fn is_same_device(&mut self, other: &FileLock) -> Result<bool> {
        if unsafe { (*self.dos_lib).lib_Version } < 37 {
            return Err(Error::UnsupportedLibraryVersion);
        }
        let res = unsafe { amiga_sys::SameDevice(self.dos_lib, self.sys_lock, other.sys_lock) };
        if res == 0 {
            return Ok(false);
        }
        Ok(true)
    }

    /// Returns true if this lock is the root directory of the file system ("zero lock").
    pub fn is_sys_root(&self) -> bool {
        self.sys_lock == 0
    }

    /// Converts this lock to [`File`].
    ///
    /// Performs an open on this lock, consuming the lock. Trying to convert a directory lock
    /// returns an error.
    ///
    /// This function calls the dos.library `OpenFromLock` function.
    #[cfg(feature = "v36")]
    pub fn to_file(mut self) -> Result<File> {
        if unsafe { (*self.dos_lib).lib_Version } < 36 {
            return Err(Error::UnsupportedLibraryVersion);
        }
        let res = unsafe { amiga_sys::OpenFromLock(self.dos_lib, self.sys_lock) };
        if res == 0 {
            // in error situations, the lock is consumed and Unlock() is called by drop
            return Err(get_ioerr(self.dos_lib));
        }
        // if open succeeds, the lock should not be UnLock()ed or used.
        self.needs_closing = false;
        Ok(File {
            dos_lib: self.dos_lib,
            file: res,
            needs_closing: true,
        })
    }

    /// Extracts the raw file lock handle.
    ///
    /// This function is typically used to borrow an owned file lock. When used in this way,
    /// this method does not pass ownership of the raw file lock handle to the caller, and
    /// the raw file lock handle is only guaranteed to be valid while the original object has
    /// not yet been destroyed.
    ///
    /// This function may return 0, which means the root of the file system.
    pub fn as_raw_handle(&self) -> RawHandle {
        self.sys_lock
    }

    /// Consumes this object, returning the underlying raw file lock handle.
    ///
    /// It is up to the caller to close the file lock handle, if it needs to be closed.
    ///
    /// This function may return 0, which means the root of the file system.
    pub fn into_raw_handle(self) -> RawHandle {
        let handle = self.sys_lock;
        core::mem::forget(self);
        handle
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        // close lock
        if !self.needs_closing {
            return;
        }
        unsafe {
            amiga_sys::UnLock(self.dos_lib, self.sys_lock);
        }
    }
}

#[repr(align(4))]
struct FileInfoBlockAligner {
    sys_fib: amiga_sys::FileInfoBlock,
}

/// Iterator to examine metadata about a file or directory.
///
/// The first item returned is metadata about a file or directory. If the first item returned
/// is a directory, subsequent items are entries under that directory.
///
/// This iterator is returned from the [`FileLock::examine()`] method and will yield instances
/// of <code>Result<[`FileInfoBlock`]></code>.
///
/// The iterator calls the dos.library `Examine` and `ExNext` functions.
pub struct Examine {
    dos_lib: *mut amiga_sys::Library,
    sys_lock: amiga_sys::BPTR,
    fiba: FileInfoBlockAligner,
    first_entry: bool,
    is_done: bool,
}

impl Iterator for Examine {
    type Item = Result<FileInfoBlock>;

    fn next(&mut self) -> Option<Self::Item> {
        // return None after the first error to prevent returning an infinite number of errors
        if self.is_done {
            return None;
        }
        let res = unsafe {
            if self.first_entry {
                self.first_entry = false;
                amiga_sys::Examine(
                    self.dos_lib,
                    self.sys_lock,
                    (&mut self.fiba.sys_fib) as *mut amiga_sys::FileInfoBlock,
                )
            } else {
                amiga_sys::ExNext(
                    self.dos_lib,
                    self.sys_lock,
                    (&mut self.fiba.sys_fib) as *mut amiga_sys::FileInfoBlock,
                )
            }
        };
        if res == 0 {
            // end of entries or error
            self.is_done = true;
            let ioerr = unsafe { amiga_sys::IoErr(self.dos_lib) };
            if ioerr == amiga_sys::ERROR_NO_MORE_ENTRIES as i32 {
                return None;
            }
            return Some(Err(Error::IoErr(ioerr)));
        }
        let file_name = self.fiba.sys_fib.fib_FileName;
        let entry_type = if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_ROOT as i32 {
            FileType::Root
        } else if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_USERDIR as i32 {
            FileType::UserDir
        } else if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_SOFTLINK as i32 {
            FileType::SoftLink
        } else if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_LINKDIR as i32 {
            FileType::LinkDir
        } else if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_FILE {
            FileType::File
        } else if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_LINKFILE {
            FileType::LinkFile
        } else if self.fiba.sys_fib.fib_DirEntryType == amiga_sys::ST_PIPEFILE {
            FileType::PipeFile
        } else {
            FileType::Unknown(self.fiba.sys_fib.fib_DirEntryType)
        };
        let modified = DateStamp {
            days: self.fiba.sys_fib.fib_Date.ds_Days,
            minutes: self.fiba.sys_fib.fib_Date.ds_Minute,
            ticks: self.fiba.sys_fib.fib_Date.ds_Tick,
        };
        let comment = self.fiba.sys_fib.fib_Comment;
        Some(Ok(FileInfoBlock {
            file_name,
            file_type: entry_type,
            protection: ProtectionBits(self.fiba.sys_fib.fib_Protection),
            len: self.fiba.sys_fib.fib_Size,
            modified,
            comment,
        }))
    }
}

/// Metadata information about a file or directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd)]
pub struct FileInfoBlock {
    /// The name of the file.
    file_name: [u8; 108],
    /// The type of the file.
    file_type: FileType,
    /// Protection bits for the file.
    protection: ProtectionBits,
    /// The size of the file in bytes.
    len: i32,
    /// The modification time.
    modified: DateStamp,
    /// The comment field.
    comment: [u8; 80],
}

impl FileInfoBlock {
    /// The name of the file.
    pub fn file_name(&self) -> &crate::CStr {
        CStr::from_bytes_until_nul(&self.file_name)
            .unwrap_or(c"")
    }

    /// The type of the file.
    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    /// Protection bits for the file.
    pub fn protection(&self) -> ProtectionBits {
        self.protection
    }

    /// The size of the file in bytes.
    ///
    /// The size of directories and the size of links to directories are unspecified.
    pub fn len(&self) -> i32 {
        self.len
    }

    /// The modification time.
    ///
    /// Which file operations update the modification time is file system dependent.
    pub fn modified(&self) -> &DateStamp {
        &self.modified
    }

    /// The comment field.
    pub fn comment(&self) -> &crate::CStr {
        CStr::from_bytes_until_nul(&self.comment)
            .unwrap_or(c"")
    }
}

/// The type of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileType {
    /// Root.
    Root,
    /// Directory.
    UserDir,
    /// Soft link pointing to a directory or a file.
    SoftLink,
    /// Hard link to a directory.
    LinkDir,
    /// File.
    File,
    /// Hard link to a file.
    LinkFile,
    /// Pipe.
    PipeFile,
    /// Unknown type.
    Unknown(i32),
}

impl FileType {
    /// Returns `true` if the type represents a directory.
    ///
    /// This is mutually exclusive to the results of `is_file` and `is_symlink`.
    /// `Unknown` values are identified as directories if the unknown value is greater than
    /// or equal to 0.
    pub fn is_dir(&self) -> bool {
        match &self {
            FileType::Root => true,
            FileType::UserDir => true,
            FileType::SoftLink => false,
            FileType::LinkDir => true,
            FileType::File => false,
            FileType::LinkFile => false,
            FileType::PipeFile => false,
            FileType::Unknown(val) => *val >= 0,
        }
    }

    /// Returns `true` if the type represents a file.
    ///
    /// This is mutually exclusive to the results of `is_dir` and `is_symlink`.
    /// `Unknown` values are identified as files if the unknown value is less than 0.
    pub fn is_file(&self) -> bool {
        match &self {
            FileType::Root => false,
            FileType::UserDir => false,
            FileType::SoftLink => false,
            FileType::LinkDir => false,
            FileType::File => true,
            FileType::LinkFile => true,
            FileType::PipeFile => true,
            FileType::Unknown(val) => *val < 0,
        }
    }

    /// Returns `true` if the type is a symbolic (soft) link.
    ///
    /// This is mutually exclusive to the results of `is_dir` and `is_file`.
    /// `Unknown` values are never identified as symbolic links.
    pub fn is_symlink(&self) -> bool {
        match &self {
            FileType::Root => false,
            FileType::UserDir => false,
            FileType::SoftLink => true,
            FileType::LinkDir => false,
            FileType::File => false,
            FileType::LinkFile => false,
            FileType::PipeFile => false,
            FileType::Unknown(_) => false,
        }
    }
}

fn get_ioerr(dos_lib: *mut amiga_sys::Library) -> Error {
    let ioerr = unsafe { amiga_sys::IoErr(dos_lib) };
    Error::IoErr(ioerr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_path() {
        assert!(validate_path(c"").is_ok());
        assert!(validate_path(c"x").is_ok());
        assert!(validate_path(c"z:").is_ok());
        assert!(validate_path(c"z:pop").is_ok());
        assert!(validate_path(c"dh0:").is_ok());
        assert!(validate_path(c"dh0:path").is_ok());
        let mut txt = [b'A'; 255];
        txt[254] = 0;
        assert!(validate_path(&CStr::from_bytes_until_nul(&txt).unwrap()).is_ok());
        let mut txt = [b'A'; 256];
        txt[255] = 0;
        assert!(validate_path(&CStr::from_bytes_until_nul(&txt).unwrap()).is_err());
    }
}
