use bitflags::bitflags;
use rand::Rng;
use std::convert::TryFrom;

pub type Mid = u64;
pub type Cid = u64;
pub type Prio = u8;

bitflags! {
    /// use promises to modify the behavior of [`Streams`].
    /// see the consts in this `struct` for
    ///
    /// [`Streams`]: crate::api::Stream
    pub struct Promises: u8 {
        /// this will guarantee that the order of messages which are send on one side,
        /// is the same when received on the other.
        const ORDERED = 0b00000001;
        /// this will guarantee that messages received haven't been altered by errors,
        /// like bit flips, this is done with a checksum.
        const CONSISTENCY = 0b00000010;
        /// this will guarantee that the other side will receive every message exactly
        /// once no messages are dropped
        const GUARANTEED_DELIVERY = 0b00000100;
        /// this will enable the internal compression on this
        /// [`Stream`](crate::api::Stream)
        #[cfg(feature = "compression")]
        const COMPRESSED = 0b00001000;
        /// this will enable the internal encryption on this
        /// [`Stream`](crate::api::Stream)
        const ENCRYPTED = 0b00010000;
    }
}

impl Promises {
    pub const fn to_le_bytes(self) -> [u8; 1] { self.bits.to_le_bytes() }
}

pub(crate) const VELOREN_MAGIC_NUMBER: [u8; 7] = [86, 69, 76, 79, 82, 69, 78]; //VELOREN
pub const VELOREN_NETWORK_VERSION: [u32; 3] = [0, 5, 0];
pub(crate) const STREAM_ID_OFFSET1: Sid = Sid::new(0);
pub(crate) const STREAM_ID_OFFSET2: Sid = Sid::new(u64::MAX / 2);

/// Support struct used for uniquely identifying [`Participant`] over the
/// [`Network`].
///
/// [`Participant`]: crate::api::Participant
/// [`Network`]: crate::api::Network
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct Pid {
    internal: u128,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub(crate) struct Sid {
    internal: u64,
}

// Used for Communication between Channel <----(TCP/UDP)----> Channel
#[derive(Debug)]
pub(crate) enum Frame {
    Handshake {
        magic_number: [u8; 7],
        version: [u32; 3],
    },
    Init {
        pid: Pid,
        secret: u128,
    },
    Shutdown, /* Shutdown this channel gracefully, if all channels are shutdown, Participant
               * is deleted */
    OpenStream {
        sid: Sid,
        prio: Prio,
        promises: Promises,
    },
    CloseStream {
        sid: Sid,
    },
    DataHeader {
        mid: Mid,
        sid: Sid,
        length: u64,
    },
    Data {
        mid: Mid,
        start: u64,
        data: Vec<u8>,
    },
    /* WARNING: Sending RAW is only used for debug purposes in case someone write a new API
     * against veloren Server! */
    Raw(Vec<u8>),
}

impl Frame {
    #[cfg(feature = "metrics")]
    pub const FRAMES_LEN: u8 = 8;

    #[cfg(feature = "metrics")]
    pub const fn int_to_string(i: u8) -> &'static str {
        match i {
            0 => "Handshake",
            1 => "Init",
            2 => "Shutdown",
            3 => "OpenStream",
            4 => "CloseStream",
            5 => "DataHeader",
            6 => "Data",
            7 => "Raw",
            _ => "",
        }
    }

    #[cfg(feature = "metrics")]
    pub fn get_int(&self) -> u8 {
        match self {
            Frame::Handshake { .. } => 0,
            Frame::Init { .. } => 1,
            Frame::Shutdown => 2,
            Frame::OpenStream { .. } => 3,
            Frame::CloseStream { .. } => 4,
            Frame::DataHeader { .. } => 5,
            Frame::Data { .. } => 6,
            Frame::Raw(_) => 7,
        }
    }

    #[cfg(feature = "metrics")]
    pub fn get_string(&self) -> &str { Self::int_to_string(self.get_int()) }

    pub fn gen_handshake(buf: [u8; 19]) -> Self {
        let magic_number = *<&[u8; 7]>::try_from(&buf[0..7]).unwrap();
        Frame::Handshake {
            magic_number,
            version: [
                u32::from_le_bytes(*<&[u8; 4]>::try_from(&buf[7..11]).unwrap()),
                u32::from_le_bytes(*<&[u8; 4]>::try_from(&buf[11..15]).unwrap()),
                u32::from_le_bytes(*<&[u8; 4]>::try_from(&buf[15..19]).unwrap()),
            ],
        }
    }

    pub fn gen_init(buf: [u8; 32]) -> Self {
        Frame::Init {
            pid: Pid::from_le_bytes(*<&[u8; 16]>::try_from(&buf[0..16]).unwrap()),
            secret: u128::from_le_bytes(*<&[u8; 16]>::try_from(&buf[16..32]).unwrap()),
        }
    }

    pub fn gen_open_stream(buf: [u8; 10]) -> Self {
        Frame::OpenStream {
            sid: Sid::from_le_bytes(*<&[u8; 8]>::try_from(&buf[0..8]).unwrap()),
            prio: buf[8],
            promises: Promises::from_bits_truncate(buf[9]),
        }
    }

    pub fn gen_close_stream(buf: [u8; 8]) -> Self {
        Frame::CloseStream {
            sid: Sid::from_le_bytes(*<&[u8; 8]>::try_from(&buf[0..8]).unwrap()),
        }
    }

    pub fn gen_data_header(buf: [u8; 24]) -> Self {
        Frame::DataHeader {
            mid: Mid::from_le_bytes(*<&[u8; 8]>::try_from(&buf[0..8]).unwrap()),
            sid: Sid::from_le_bytes(*<&[u8; 8]>::try_from(&buf[8..16]).unwrap()),
            length: u64::from_le_bytes(*<&[u8; 8]>::try_from(&buf[16..24]).unwrap()),
        }
    }

    pub fn gen_data(buf: [u8; 18]) -> (Mid, u64, u16) {
        let mid = Mid::from_le_bytes(*<&[u8; 8]>::try_from(&buf[0..8]).unwrap());
        let start = u64::from_le_bytes(*<&[u8; 8]>::try_from(&buf[8..16]).unwrap());
        let length = u16::from_le_bytes(*<&[u8; 2]>::try_from(&buf[16..18]).unwrap());
        (mid, start, length)
    }

    pub fn gen_raw(buf: [u8; 2]) -> u16 {
        u16::from_le_bytes(*<&[u8; 2]>::try_from(&buf[0..2]).unwrap())
    }
}

impl Pid {
    /// create a new Pid with a random interior value
    ///
    /// # Example
    /// ```rust
    /// use veloren_network::{Network, Pid};
    ///
    /// let pid = Pid::new();
    /// let _ = Network::new(pid);
    /// ```
    pub fn new() -> Self {
        Self {
            internal: rand::thread_rng().gen(),
        }
    }

    /// don't use fake! just for testing!
    /// This will panic if pid i greater than 7, as I do not want you to use
    /// this in production!
    #[doc(hidden)]
    pub fn fake(pid_offset: u8) -> Self {
        assert!(pid_offset < 8);
        let o = pid_offset as u128;
        const OFF: [u128; 5] = [
            0x40,
            0x40 * 0x40,
            0x40 * 0x40 * 0x40,
            0x40 * 0x40 * 0x40 * 0x40,
            0x40 * 0x40 * 0x40 * 0x40 * 0x40,
        ];
        Self {
            internal: o + o * OFF[0] + o * OFF[1] + o * OFF[2] + o * OFF[3] + o * OFF[4],
        }
    }

    pub(crate) fn to_le_bytes(&self) -> [u8; 16] { self.internal.to_le_bytes() }

    pub(crate) fn from_le_bytes(bytes: [u8; 16]) -> Self {
        Self {
            internal: u128::from_le_bytes(bytes),
        }
    }
}

impl Sid {
    pub const fn new(internal: u64) -> Self { Self { internal } }

    pub(crate) fn to_le_bytes(&self) -> [u8; 8] { self.internal.to_le_bytes() }

    pub(crate) fn from_le_bytes(bytes: [u8; 8]) -> Self {
        Self {
            internal: u64::from_le_bytes(bytes),
        }
    }
}

impl std::fmt::Debug for Pid {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const BITS_PER_SIXLET: usize = 6;
        //only print last 6 chars of number as full u128 logs are unreadable
        const CHAR_COUNT: usize = 6;
        for i in 0..CHAR_COUNT {
            write!(
                f,
                "{}",
                sixlet_to_str((self.internal >> (i * BITS_PER_SIXLET)) & 0x3F)
            )?;
        }
        Ok(())
    }
}

impl Default for Pid {
    fn default() -> Self { Pid::new() }
}

impl std::fmt::Display for Pid {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{:?}", self) }
}

impl std::ops::AddAssign for Sid {
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            internal: self.internal + other.internal,
        };
    }
}

impl std::fmt::Debug for Sid {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        //only print last 6 chars of number as full u128 logs are unreadable
        write!(f, "{}", self.internal.rem_euclid(1000000))
    }
}

impl From<u64> for Sid {
    fn from(internal: u64) -> Self { Sid { internal } }
}

impl std::fmt::Display for Sid {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.internal)
    }
}

fn sixlet_to_str(sixlet: u128) -> char {
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"[sixlet as usize] as char
}

#[cfg(test)]
mod tests {
    use crate::types::*;

    #[test]
    fn frame_int2str() {
        assert_eq!(Frame::int_to_string(3), "OpenStream");
        assert_eq!(Frame::int_to_string(7), "Raw");
        assert_eq!(Frame::int_to_string(8), "");
    }

    #[test]
    fn frame_get_int() {
        assert_eq!(Frame::get_int(&Frame::Raw(b"Foo".to_vec())), 7);
        assert_eq!(Frame::get_int(&Frame::Shutdown), 2);
    }

    #[test]
    fn frame_creation() {
        Pid::new();
        assert_eq!(format!("{}", Pid::fake(0)), "AAAAAA");
        assert_eq!(format!("{}", Pid::fake(1)), "BBBBBB");
        assert_eq!(format!("{}", Pid::fake(2)), "CCCCCC");
    }

    #[test]
    fn test_sixlet_to_str() {
        assert_eq!(sixlet_to_str(0), 'A');
        assert_eq!(sixlet_to_str(29), 'd');
        assert_eq!(sixlet_to_str(63), '/');
    }
}
