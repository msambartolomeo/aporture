use crate::{crypto::cipher::Cipher, parser::Parser};
use bytes::{Buf, BufMut};

const LENGHT_SIZE: usize = 2;
const BYTE_SIZE: usize = 1;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct Message {
    length: [u8; LENGHT_SIZE],
    encrypted: [u8; BYTE_SIZE],
    content: Content,
}

#[derive(Debug, Clone)]
pub enum Content {
    Plain {
        content: Vec<u8>,
    },
    Encrypted {
        nonce: [u8; NONCE_SIZE],
        content: Vec<u8>,
        tag: [u8; TAG_SIZE],
    },
}

impl Message {
    #[allow(clippy::cast_possible_truncation)] // As truncation is checked explicitly
    pub fn new(parser: &impl Parser, cipher: Option<&Cipher>) -> Self {
        let mut content = parser.serialize_to();

        let length = content.len();

        assert!(
            length < u16::MAX.into(),
            "Message Payload must be smaller than u16::MAX"
        );

        let length = (length as u16).to_be_bytes();

        match cipher {
            Some(cipher) => {
                let (nonce, tag) = cipher.encrypt(&mut content);

                Self {
                    length,
                    encrypted: [1],
                    content: Content::Encrypted {
                        nonce,
                        content,
                        tag,
                    },
                }
            }
            None => Self {
                length,
                encrypted: [0],
                content: Content::Plain { content },
            },
        }
    }

    #[must_use]
    pub fn into_buf(self) -> MessageBuffer {
        self.into()
    }

    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.encrypted == [1]
    }

    fn content_mut(&mut self) -> &mut Vec<u8> {
        match &mut self.content {
            Content::Plain { content, .. } | Content::Encrypted { content, .. } => content,
        }
    }

    fn length(&self) -> usize {
        u16::from_be_bytes(self.length).into()
    }

    fn total_remaining(&self, state: State) -> usize {
        match state {
            State::Length => LENGHT_SIZE + BYTE_SIZE,
            State::Encrypt => BYTE_SIZE,
            State::Nonce if self.is_encrypted() => NONCE_SIZE + self.length() + TAG_SIZE,
            State::Content if self.is_encrypted() => self.length() + TAG_SIZE,
            State::Tag if self.is_encrypted() => TAG_SIZE,
            State::Content /* if !self.is_encrypted() */ => self.length(),
            State::Nonce | State::Tag => unreachable!("Should not be possible unencrypted"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Length,
    Encrypt,
    Nonce,
    Content,
    Tag,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct MessageBuffer {
    message: Message,
    state: State,
    cursor: usize,
}

impl From<Message> for MessageBuffer {
    fn from(message: Message) -> Self {
        Self {
            message,
            state: State::Length,
            cursor: 0,
        }
    }
}

impl Buf for MessageBuffer {
    fn remaining(&self) -> usize {
        let total_remaining = self.message.total_remaining(self.state);

        total_remaining - self.cursor
    }

    fn chunk(&self) -> &[u8] {
        let slice: &[u8] = if self.message.is_encrypted() {
            let Content::Encrypted {
                nonce,
                content,
                tag,
            } = &self.message.content
            else {
                unreachable!("Content is marked as encrypted");
            };

            match self.state {
                State::Length => &self.message.length,
                State::Encrypt => &self.message.encrypted,
                State::Content => content,
                State::Nonce => nonce,
                State::Tag => tag,
            }
        } else {
            let Content::Plain { ref content } = self.message.content else {
                unreachable!("Content is marked as unencrypted");
            };

            match self.state {
                State::Length => &self.message.length,
                State::Encrypt => &self.message.encrypted,
                State::Content => content,
                _ => unreachable!("Plain cannot have other states"),
            }
        };

        &slice[self.cursor..]
    }

    fn advance(&mut self, mut cnt: usize) {
        assert!(cnt < self.remaining());

        loop {
            let chunk_lenght = self.chunk().len();

            if cnt < chunk_lenght {
                self.cursor += cnt;
                break;
            }

            cnt -= chunk_lenght;
            self.cursor = 0;

            self.state = match self.state {
                State::Length => State::Encrypt,
                State::Encrypt => {
                    if self.message.is_encrypted() {
                        State::Nonce
                    } else {
                        State::Content
                    }
                }
                State::Nonce => State::Content,
                State::Content => {
                    if self.message.is_encrypted() {
                        State::Tag
                    } else {
                        unreachable!("Already asserted cnt < remaining");
                    }
                }
                State::Tag => unreachable!("Already asserted cnt < remaining"),
            }
        }
    }
}

unsafe impl BufMut for MessageBuffer {
    fn remaining_mut(&self) -> usize {
        let total_remaining = self.message.total_remaining(self.state);

        total_remaining - self.cursor
    }

    unsafe fn advance_mut(&mut self, mut cnt: usize) {
        assert!(cnt < self.remaining());

        loop {
            let chunk_lenght = self.chunk_mut().len();

            if cnt < chunk_lenght {
                self.cursor += cnt;
                if matches!(self.state, State::Content) {
                    // SAFETY: In length state capacity was set, so in content state we can
                    // set the len of the content vec while we are still in the same chunk, as it
                    // will be less than the capacity and message length.
                    unsafe {
                        self.message.content_mut().set_len(self.cursor);
                    }
                }
                break;
            }

            cnt -= chunk_lenght;
            self.cursor = 0;

            self.state = match self.state {
                State::Length => {
                    let content_length = self.message.length();

                    self.message.content_mut().reserve(content_length);

                    State::Encrypt
                }
                State::Encrypt => {
                    if self.message.is_encrypted() {
                        State::Nonce
                    } else {
                        State::Content
                    }
                }
                State::Nonce => State::Content,
                State::Content => {
                    if self.message.is_encrypted() {
                        State::Tag
                    } else {
                        unreachable!("Already asserted cnt < remaining");
                    }
                }
                State::Tag => unreachable!("Already asserted cnt < remaining"),
            }
        }
    }

    fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
        match self.state {
            State::Length => bytes::buf::UninitSlice::new(&mut self.message.length),
            State::Encrypt => bytes::buf::UninitSlice::new(&mut self.message.encrypted),
            State::Nonce => {
                let Content::Encrypted { nonce, .. } = &mut self.message.content else {
                    unreachable!("State nonce means content is marked as encrypted");
                };

                bytes::buf::UninitSlice::new(nonce)
            }
            State::Content => {
                let content = self.message.content_mut();

                debug_assert_eq!(self.cursor, content.len());

                let ptr = content.as_mut_ptr();

                // SAFETY: Cursor is equal to len and the available size is marked by
                // the expected message lenght minus the cursor which is allocated in advance_mut.
                unsafe {
                    bytes::buf::UninitSlice::from_raw_parts_mut(
                        ptr.add(self.cursor),
                        self.message.length() - self.cursor,
                    )
                }
            }
            State::Tag => {
                let Content::Encrypted { tag, .. } = &mut self.message.content else {
                    unreachable!("State tag means content is marked as encrypted");
                };
                bytes::buf::UninitSlice::new(tag)
            }
        }
    }
}
