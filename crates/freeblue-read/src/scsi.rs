//! Minimal Linux `SG_IO` SCSI pass-through (spec 11 §11.4.7).
//!
//! Just the two commands the LibreDrive raw-read path needs: **READ BUFFER**
//! (`0x3C`, the LibreDrive unlock primitive) and **READ(10)** (`0x28`, content).
//! All transfers are drive→host (`SG_DXFER_FROM_DEV`). No external SCSI crate —
//! a direct `ioctl(fd, SG_IO, &sg_io_hdr)`. Linux-only; the rest of the crate
//! builds everywhere.

#![cfg(target_os = "linux")]

use std::io;
use std::os::unix::io::RawFd;

const SG_IO: libc::c_ulong = 0x2285;
const SG_DXFER_FROM_DEV: libc::c_int = -3;
const SG_INFO_OK_MASK: u32 = 0x1;
const SG_INFO_OK: u32 = 0x0;

/// Linux `sg_io_hdr_t` (v3 interface), `repr(C)` to match the kernel ABI.
#[repr(C)]
struct SgIoHdr {
    interface_id: libc::c_int,
    dxfer_direction: libc::c_int,
    cmd_len: libc::c_uchar,
    mx_sb_len: libc::c_uchar,
    iovec_count: libc::c_ushort,
    dxfer_len: libc::c_uint,
    dxferp: *mut libc::c_void,
    cmdp: *const libc::c_uchar,
    sbp: *mut libc::c_uchar,
    timeout: libc::c_uint,
    flags: libc::c_uint,
    pack_id: libc::c_int,
    usr_ptr: *mut libc::c_void,
    status: libc::c_uchar,
    masked_status: libc::c_uchar,
    msg_status: libc::c_uchar,
    sb_len_wr: libc::c_uchar,
    host_status: libc::c_ushort,
    driver_status: libc::c_ushort,
    resid: libc::c_int,
    duration: libc::c_uint,
    info: libc::c_uint,
}

/// An open optical drive we can issue raw SCSI to.
pub struct ScsiDev {
    fd: RawFd,
}

impl ScsiDev {
    /// Open a drive (e.g. `/dev/sr0`) for SCSI pass-through.
    pub fn open(path: &str) -> io::Result<Self> {
        let c = std::ffi::CString::new(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "device path has NUL"))?;
        // O_NONBLOCK so opening a drive with no/mounting media doesn't hang.
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(ScsiDev { fd })
    }

    /// Issue one drive→host CDB, returning `dxfer_len` bytes (minus any residual).
    // `from_dev` names the SCSI transfer direction (SG_DXFER_FROM_DEV), not a
    // constructor — the `wrong_self_convention` lint doesn't apply here.
    #[allow(clippy::wrong_self_convention)]
    fn from_dev(&self, cdb: &[u8], dxfer_len: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; dxfer_len];
        let mut sense = [0u8; 64];
        let mut hdr: SgIoHdr = unsafe { std::mem::zeroed() };
        hdr.interface_id = b'S' as libc::c_int;
        hdr.dxfer_direction = SG_DXFER_FROM_DEV;
        hdr.cmd_len = cdb.len() as libc::c_uchar;
        hdr.mx_sb_len = sense.len() as libc::c_uchar;
        hdr.dxfer_len = dxfer_len as libc::c_uint;
        hdr.dxferp = buf.as_mut_ptr() as *mut libc::c_void;
        hdr.cmdp = cdb.as_ptr();
        hdr.sbp = sense.as_mut_ptr();
        hdr.timeout = 30_000; // ms

        let rc = unsafe { libc::ioctl(self.fd, SG_IO, &mut hdr as *mut SgIoHdr) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        // info carries the overall result; status/host/driver must be clean.
        if (hdr.info & SG_INFO_OK_MASK) != SG_INFO_OK
            || hdr.status != 0
            || hdr.host_status != 0
            || hdr.driver_status != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "SCSI cmd {:#04x} failed: status={:#x} host={:#x} driver={:#x} sense={:02x?}",
                    cdb.first().copied().unwrap_or(0),
                    hdr.status,
                    hdr.host_status,
                    hdr.driver_status,
                    &sense[..sense.len().min(8)]
                ),
            ));
        }
        let got = dxfer_len.saturating_sub(hdr.resid.max(0) as usize);
        buf.truncate(got);
        Ok(buf)
    }

    /// READ BUFFER (`0x3C`): `mode`/`buffer_id`-selected drive memory window.
    /// LibreDrive uses mode 2, buffer-id `0x77` (spec 11 §11.4).
    pub fn read_buffer(
        &self,
        mode: u8,
        buffer_id: u8,
        offset: u32,
        len: u16,
    ) -> io::Result<Vec<u8>> {
        let cdb = [
            0x3C,
            mode & 0x1F,
            buffer_id,
            (offset >> 16) as u8,
            (offset >> 8) as u8,
            offset as u8,
            0x00, // allocation length is 24-bit; a u16 len never sets the top byte
            (len >> 8) as u8,
            len as u8,
            0x00,
        ];
        self.from_dev(&cdb, len as usize)
    }

    /// READ(10) (`0x28`): `sectors` × 2048-byte logical blocks from `lba`.
    pub fn read10(&self, lba: u32, sectors: u16) -> io::Result<Vec<u8>> {
        let cdb = [
            0x28,
            0x00,
            (lba >> 24) as u8,
            (lba >> 16) as u8,
            (lba >> 8) as u8,
            lba as u8,
            0x00,
            (sectors >> 8) as u8,
            sectors as u8,
            0x00,
        ];
        self.from_dev(&cdb, sectors as usize * 2048)
    }
}

impl Drop for ScsiDev {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
