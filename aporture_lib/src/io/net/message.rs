use std::marker::PhantomData;

use crate::{crypto::cipher::Cipher, parser::Parser};
use bytes::{Buf, BufMut};

const LENGHT_SIZE: usize = 2;
const FLAG_SIZE: usize = 1;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub struct Message<T> {
    length: [u8; LENGHT_SIZE],
    encrypted: [u8; FLAG_SIZE],
    content: Content,
    _phantom: PhantomData<T>,
}

impl<T> Default for Message<T> {
    fn default() -> Self {
        Self {
            length: Default::default(),
            encrypted: Default::default(),
            content: Default::default(),
            _phantom: PhantomData,
        }
    }
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

impl Default for Content {
    fn default() -> Self {
        Self::Plain {
            content: Vec::with_capacity(0),
        }
    }
}

impl<T> Message<T>
where
    T: Parser,
{
    #[allow(clippy::cast_possible_truncation)] // As truncation is checked explicitly
    pub fn new(parser: &T, cipher: Option<&Cipher>) -> Self {
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
                    _phantom: PhantomData,
                }
            }
            None => Self {
                length,
                encrypted: [0],
                content: Content::Plain { content },
                _phantom: PhantomData,
            },
        }
    }

    pub fn consume(self, cipher: Option<&Cipher>) -> Result<T, crate::io::Error> {
        match (self.content, cipher) {
            (
                Content::Encrypted {
                    ref nonce,
                    ref mut content,
                    ref tag,
                },
                Some(c),
            ) => {
                c.decrypt(content, nonce, tag)?;

                Ok(T::deserialize_from(&content).unwrap())
            }
            (Content::Plain { ref content }, None) => Ok(T::deserialize_from(content).unwrap()),
            _ => Err(crate::io::Error::Cipher(crate::crypto::Error::Decrypt)),
        }
    }
}

impl<T> Message<T> {
    #[must_use]
    pub fn into_buf(self) -> MessageBuffer<T> {
        MessageBuffer::new(self)
    }

    pub fn empty() -> Self {
        Self::default()
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

    pub fn content(&self) -> &[u8] {
        match &self.content {
            Content::Plain { content, .. } | Content::Encrypted { content, .. } => content,
        }
    }

    fn length(&self) -> usize {
        u16::from_be_bytes(self.length).into()
    }

    fn total_remaining(&self, state: State) -> usize {
        match state {
            State::Length => LENGHT_SIZE + FLAG_SIZE,
            State::Encrypt => FLAG_SIZE,
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
pub struct MessageBuffer<T> {
    message: Message<T>,
    state: State,
    cursor: usize,
}

impl<T> MessageBuffer<T> {
    pub fn new(message: Message<T>) -> Self {
        Self {
            message,
            state: State::Length,
            cursor: 0,
        }
    }

    pub fn into_inner(self) -> Message<T> {
        self.message
    }
}

impl<T> From<MessageBuffer<T>> for Message<T> {
    fn from(buffer: MessageBuffer<T>) -> Self {
        buffer.into_inner()
    }
}

impl<T> From<Message<T>> for MessageBuffer<T> {
    fn from(message: Message<T>) -> Self {
        Self::new(message)
    }
}

impl<T> Buf for MessageBuffer<T> {
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
        assert!(cnt <= self.remaining());

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
                        self.cursor = self.message.length();
                        break;
                    }
                }
                State::Tag => {
                    self.cursor = TAG_SIZE;
                    break;
                }
            }
        }
    }
}

unsafe impl<T> BufMut for MessageBuffer<T> {
    fn remaining_mut(&self) -> usize {
        let total_remaining = self.message.total_remaining(self.state);

        total_remaining - self.cursor
    }

    unsafe fn advance_mut(&mut self, mut cnt: usize) {
        assert!(cnt <= self.remaining());

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
                State::Length => State::Encrypt,
                State::Encrypt => {
                    let content_length = self.message.length();

                    if self.message.is_encrypted() {
                        self.message.content = Content::Encrypted {
                            nonce: Default::default(),
                            content: Vec::with_capacity(content_length),
                            tag: Default::default(),
                        };

                        State::Nonce
                    } else {
                        self.message.content = Content::Plain {
                            content: Vec::with_capacity(content_length),
                        };

                        State::Content
                    }
                }
                State::Nonce => State::Content,
                State::Content => {
                    let content_length = self.message.length();

                    // SAFETY: Vec was initialized with capacity content_length and
                    // If state Content has finished then all the content was already written
                    unsafe {
                        self.message.content_mut().set_len(content_length);
                    }

                    if self.message.is_encrypted() {
                        State::Tag
                    } else {
                        self.cursor = self.message.length();
                        break;
                    }
                }
                State::Tag => {
                    self.cursor = TAG_SIZE;
                    break;
                }
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

#[cfg(test)]
mod test {
    use std::io::Read;
    use std::io::Write;

    use crate::protocol::Hello;
    use crate::protocol::PairKind;

    use super::*;

    #[test]
    fn new() {
        let hello = Hello {
            version: 1,
            kind: PairKind::Sender,
            pair_id: [b'a'; 32],
        };

        let message = Message::new(&hello, None);

        let content = hello.serialize_to();

        assert_eq!(false, message.is_encrypted());
        assert_eq!(content.len(), message.length());
        assert_eq!(content, *message.content());
        assert_eq!(hello, message.consume(None).unwrap())
    }

    #[test]
    fn reading() -> Result<(), Box<dyn std::error::Error>> {
        let input = PairKind::Sender;

        let message = Message::new(&input, None);

        let buf = message.into_buf();
        let mut reader = buf.reader();

        let mut output = [0; 64];

        let mut ptr = &mut output[..];

        let mut n;
        let mut len = 0;
        loop {
            n = reader.read(ptr)?;

            if n == 0 {
                break;
            }

            len += n;

            ptr = &mut ptr[n..];
        }

        let serialized = input.serialize_to();

        assert_eq!(LENGHT_SIZE + FLAG_SIZE + serialized.len(), len);
        assert_eq!(3u16.to_be_bytes(), output[..2]);
        assert_eq!(0, output[2]);
        assert_eq!(input.serialize_to(), output[3..len]);

        Ok(())
    }

    #[test]
    fn writing() -> Result<(), Box<dyn std::error::Error>> {
        let input = [0, 3, 0, 105, 48, 101];

        let message = Message::<PairKind>::default();

        let buf = message.into_buf();
        let mut writer = buf.writer();

        let mut ptr = &input[..];

        let mut n;
        let mut len = 0;
        loop {
            n = writer.write(ptr)?;

            if n == 0 {
                break;
            }

            len += n;

            ptr = &ptr[n..];
        }

        let buf = writer.into_inner();
        let message = buf.into_inner();

        assert_eq!(input.len(), len);
        assert_eq!(3u16.to_be_bytes(), message.length);
        assert_eq!([0], message.encrypted);
        assert_eq!(&input[3..], message.content());

        let output = message.consume(None)?;
        assert_eq!(PairKind::Sender, output);

        Ok(())
    }

    #[test]
    fn new_encrypted() {
        let hello = Hello {
            version: 1,
            kind: PairKind::Sender,
            pair_id: [b'a'; 32],
        };

        let cipher = Cipher::new(&[b'a'; 32]);

        let message = Message::new(&hello, Some(&cipher));

        let content = hello.serialize_to();

        assert_eq!(true, message.is_encrypted());
        assert_eq!(content.len(), message.length());
        assert_eq!(hello, message.consume(Some(&cipher)).unwrap())
    }

    #[test]
    fn reading_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let input = PairKind::Sender;

        let cipher = Cipher::new(&[b'a'; 32]);

        let message = Message::new(&input, Some(&cipher));

        let content_length = message.length();

        let buf = message.into_buf();
        let mut reader = buf.reader();

        let mut output = [0; 64];

        let mut ptr = &mut output[..];

        let mut n;
        let mut len = 0;
        loop {
            n = reader.read(ptr)?;

            if n == 0 {
                break;
            }

            len += n;

            ptr = &mut ptr[n..];
        }

        let serialized = input.serialize_to();

        let (length, rest) = output.split_at_mut(LENGHT_SIZE);
        let (encrypt, rest) = rest.split_at_mut(FLAG_SIZE);
        let (nonce, rest) = rest.split_at_mut(NONCE_SIZE);
        let (content, rest) = rest.split_at_mut(content_length);
        let (tag, _) = rest.split_at_mut(TAG_SIZE);

        assert_eq!(
            LENGHT_SIZE + FLAG_SIZE + NONCE_SIZE + serialized.len() + TAG_SIZE,
            len
        );
        assert_eq!(3u16.to_be_bytes(), length);
        assert_eq!([1], encrypt);

        let nonce = nonce.try_into()?;
        let tag = tag.try_into()?;

        cipher.decrypt(content, &nonce, &tag)?;

        assert_eq!(serialized, content);

        Ok(())
    }

    #[test]
    fn writing_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let input = [
            0, 3, 1, 168, 55, 51, 191, 5, 87, 80, 203, 213, 81, 13, 15, 20, 201, 214, 149, 206,
            132, 235, 56, 53, 21, 90, 164, 194, 175, 15, 29, 109, 181, 27,
        ];

        let cipher = Cipher::new(&[b'a'; 32]);

        let message = Message::<PairKind>::default();

        let buf = message.into_buf();
        let mut writer = buf.writer();

        let mut ptr = &input[..];

        let mut n;
        let mut len = 0;
        loop {
            n = writer.write(ptr)?;

            if n == 0 {
                break;
            }

            len += n;

            ptr = &ptr[n..];
        }

        let buf = writer.into_inner();
        let message = buf.into_inner();

        assert_eq!(input.len(), len);
        assert_eq!(3u16.to_be_bytes(), message.length);
        assert_eq!([1], message.encrypted);

        let output = message.consume(Some(&cipher))?;
        assert_eq!(PairKind::Sender, output);

        Ok(())
    }
}
