#[cfg(feature = "full")]
use crate::crypto::cipher::Cipher;
use bytes::{Buf, BufMut};
use thiserror::Error;

const LENGTH_SIZE: usize = 2;
const FLAG_SIZE: usize = 1;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

#[derive(Debug)]
pub struct Message<'a> {
    length: [u8; LENGTH_SIZE],
    encrypted: EncryptedContent,
    content: &'a mut [u8],
}

#[derive(Debug, Clone)]
pub enum EncryptedContent {
    Plain {
        bit: [u8; FLAG_SIZE],
    },
    Encrypted {
        bit: [u8; FLAG_SIZE],
        nonce: [u8; NONCE_SIZE],
        tag: [u8; TAG_SIZE],
    },
}

impl EncryptedContent {
    #[must_use]
    const fn plain() -> Self {
        Self::Plain { bit: [0] }
    }

    #[must_use]
    const fn encrypted(nonce: [u8; NONCE_SIZE], tag: [u8; TAG_SIZE]) -> Self {
        Self::Encrypted {
            bit: [1],
            nonce,
            tag,
        }
    }
}

impl<'a> Message<'a> {
    #[allow(clippy::cast_possible_truncation)] // As truncation is checked explicitly
    pub fn new(content: &'a mut [u8]) -> Self {
        let length = content.len();

        assert!(
            length <= u16::MAX.into(),
            "Message Payload must be smaller than u16::MAX"
        );

        let length = (length as u16).to_be_bytes();

        Self {
            length,
            encrypted: EncryptedContent::plain(),
            content,
        }
    }

    #[cfg(feature = "full")]
    #[allow(clippy::cast_possible_truncation)] // As truncation is checked explicitly
    pub fn new_encrypted(content: &'a mut [u8], cipher: &Cipher) -> Self {
        let length = content.len();

        assert!(
            length <= u16::MAX.into(),
            "Message Payload must be smaller than u16::MAX"
        );

        let length = (length as u16).to_be_bytes();

        let (nonce, tag) = cipher.encrypt(content);

        Self {
            length,
            encrypted: EncryptedContent::encrypted(nonce, tag),
            content,
        }
    }

    #[must_use]
    pub const fn into_buf(self) -> MessageBuffer<'a> {
        MessageBuffer::new(self)
    }

    #[must_use]
    pub const fn is_encrypted(&self) -> bool {
        match self.encrypted {
            EncryptedContent::Plain { .. } => false,
            EncryptedContent::Encrypted { .. } => true,
        }
    }

    #[must_use]
    pub const fn get_encryption_bit(&self) -> [u8; 1] {
        match self.encrypted {
            EncryptedContent::Plain { bit } | EncryptedContent::Encrypted { bit, .. } => bit,
        }
    }

    fn length(&self) -> usize {
        u16::from_be_bytes(self.length).into()
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
#[derive(Debug)]
pub struct MessageBuffer<'a> {
    message: Message<'a>,
    state: State,
    cursor: usize,
    error: Option<ErrorKind>,
}

#[derive(Debug, Error, Clone, Copy)]
pub enum ErrorKind {
    #[error("Message is encrypted but no cipher was provided")]
    CipherExpected,
    #[cfg(feature = "full")]
    #[error("Error decrypting message")]
    Decryption(#[from] crate::crypto::Error),
    #[error("The provided buffer was not sufficient to read all the data")]
    InsufficientBuffer,
    #[error("The message received is invalid")]
    InvalidMessage,
}

#[derive(Debug, Error)]
#[error("{0}")]
pub struct Error<'a>(pub ErrorKind, pub MessageBuffer<'a>);

impl Error<'_> {
    #[must_use]
    pub const fn ignore(self) -> ErrorKind {
        self.0
    }
}

impl<'a> From<Error<'a>> for ErrorKind {
    fn from(Error(error, _): Error<'a>) -> Self {
        error
    }
}

impl<'a> MessageBuffer<'a> {
    #[must_use]
    pub const fn new(message: Message<'a>) -> Self {
        Self {
            message,
            state: State::Length,
            cursor: 0,
            error: None,
        }
    }

    pub fn consume(self) -> Result<usize, Error<'a>> {
        if let Some(err) = self.error {
            return Err(Error(err, self));
        }

        let length = self.message.length();

        match &self.message.encrypted {
            EncryptedContent::Plain { .. } => (),
            EncryptedContent::Encrypted { .. } => {
                return Err(Error(ErrorKind::CipherExpected, self))
            }
        }

        Ok(length)
    }

    #[cfg(feature = "full")]
    pub fn consume_encrypted(self, cipher: &Cipher) -> Result<usize, Error<'a>> {
        if let Some(err) = self.error {
            return Err(Error(err, self));
        }

        let length = self.message.length();
        let content = &mut self.message.content[..length];

        match &self.message.encrypted {
            EncryptedContent::Encrypted { nonce, tag, .. } => {
                cipher
                    .decrypt(content, nonce, tag)
                    .map_err(move |e| Error(e.into(), self))?;
            }
            EncryptedContent::Plain { .. } => (),
        }

        Ok(length)
    }

    fn total_remaining(&self, state: State) -> usize {
        match state {
            State::Length => LENGTH_SIZE + FLAG_SIZE,
            State::Encrypt => FLAG_SIZE,
            State::Nonce if self.message.is_encrypted() => NONCE_SIZE + self.message.length() + TAG_SIZE,
            State::Content if self.message.is_encrypted() => self.message.length(),
            State::Tag if self.message.is_encrypted() => TAG_SIZE,
            State::Content /* if !self.is_encrypted() */ => self.message.length(),
            State::Nonce | State::Tag => unreachable!("Should not be possible unencrypted"),
        }
    }
}

impl<'a> From<Message<'a>> for MessageBuffer<'a> {
    fn from(message: Message<'a>) -> Self {
        Self::new(message)
    }
}

impl Buf for MessageBuffer<'_> {
    fn remaining(&self) -> usize {
        let total_remaining = self.total_remaining(self.state);

        total_remaining - self.cursor
    }

    fn chunk(&self) -> &[u8] {
        let slice: &[u8] = match &self.message.encrypted {
            EncryptedContent::Plain { bit } => match self.state {
                State::Length => &self.message.length,
                State::Encrypt => bit,
                State::Content => self.message.content,
                _ => unreachable!("Plain cannot have other states"),
            },
            EncryptedContent::Encrypted { nonce, tag, bit } => match self.state {
                State::Length => &self.message.length,
                State::Encrypt => bit,
                State::Content => self.message.content,
                State::Nonce => nonce,
                State::Tag => tag,
            },
        };

        &slice[self.cursor..]
    }

    fn advance(&mut self, mut cnt: usize) {
        assert!(cnt <= self.remaining());

        loop {
            let chunk_length = self.chunk().len();

            if cnt < chunk_length {
                self.cursor += cnt;
                break;
            }

            cnt -= chunk_length;
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

unsafe impl BufMut for MessageBuffer<'_> {
    fn remaining_mut(&self) -> usize {
        if self.error.is_some() {
            return 0;
        }

        let total_remaining = self.total_remaining(self.state);

        total_remaining - self.cursor
    }

    unsafe fn advance_mut(&mut self, mut cnt: usize) {
        assert!(cnt <= self.remaining_mut());

        loop {
            let chunk_length = self.chunk_mut().len();

            if cnt < chunk_length {
                self.cursor += cnt;
                break;
            }

            cnt -= chunk_length;
            self.cursor = 0;

            self.state = match self.state {
                State::Length => State::Encrypt,
                State::Encrypt => {
                    let content_length = self.message.length();
                    let available_length = self.message.content.len();
                    if available_length < content_length {
                        self.error = Some(ErrorKind::InsufficientBuffer);
                        break;
                    }

                    match self.message.get_encryption_bit() {
                        [0] => {
                            self.message.encrypted = EncryptedContent::plain();

                            State::Content
                        }
                        [1] => {
                            self.message.encrypted =
                                EncryptedContent::encrypted([0; NONCE_SIZE], [0; TAG_SIZE]);

                            State::Nonce
                        }
                        _ => {
                            self.error = Some(ErrorKind::InvalidMessage);
                            break;
                        }
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

    fn chunk_mut(&mut self) -> &mut bytes::buf::UninitSlice {
        let length = self.message.length();

        let slice: &mut [u8] = match &mut self.message.encrypted {
            EncryptedContent::Plain { bit } => match self.state {
                State::Length => &mut self.message.length,
                State::Encrypt => bit,
                State::Content => &mut self.message.content[..length],
                _ => unreachable!("Plain cannot have other states"),
            },
            EncryptedContent::Encrypted { nonce, tag, bit } => match self.state {
                State::Length => &mut self.message.length,
                State::Encrypt => bit,
                State::Content => &mut self.message.content[..length],
                State::Nonce => nonce,
                State::Tag => tag,
            },
        };

        bytes::buf::UninitSlice::new(&mut slice[self.cursor..])
    }
}

#[cfg(test)]
mod test {
    use std::io::Read;
    use std::io::Write;

    use super::*;

    #[test]
    fn new() {
        let hello = b"Hello";

        let mut input = *hello;

        let message = Message::new(&mut input);

        assert!(!message.is_encrypted());
        assert_eq!(hello.len(), message.length());
        assert_eq!(hello, message.content);
    }

    #[test]
    fn reading() -> Result<(), Box<dyn std::error::Error>> {
        let hello = b"Hello";

        let mut input = *hello;

        let message = Message::new(&mut input);

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

        assert_eq!(LENGTH_SIZE + FLAG_SIZE + hello.len(), len);
        assert_eq!(u16::try_from(hello.len())?.to_be_bytes(), output[..2]);
        assert_eq!(0, output[2]);
        assert_eq!(hello, &output[3..len]);

        Ok(())
    }

    #[test]
    fn writing() -> Result<(), Box<dyn std::error::Error>> {
        let input = [0, 5, 0, 72, 101, 108, 108, 111];

        let mut buffer = [0; 1000];

        let message = Message::new(&mut buffer);

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
        let message = &buf.message;

        assert_eq!(input.len(), len);
        assert_eq!(input[..2], message.length);
        assert_eq!([0], message.get_encryption_bit());

        let n = buf.consume().map_err(Error::ignore)?;

        assert_eq!(&input[3..], &buffer[..n]);
        assert_eq!(b"Hello", &buffer[..n]);

        Ok(())
    }

    #[test]
    fn new_encrypted() {
        let hello = b"Hello";

        let mut input = *hello;

        let cipher = Cipher::new(&[b'a'; 32]);

        let message = Message::new_encrypted(&mut input, &cipher);

        assert!(message.is_encrypted());
        assert_eq!(hello.len(), message.length());
    }

    #[test]
    fn reading_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let hello = b"Hello";

        let mut input = *hello;

        let cipher = Cipher::new(&[b'a'; 32]);

        let message = Message::new_encrypted(&mut input, &cipher);

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

        let (length, rest) = output.split_at_mut(LENGTH_SIZE);
        let (encrypt, rest) = rest.split_at_mut(FLAG_SIZE);
        let (nonce, rest) = rest.split_at_mut(NONCE_SIZE);
        let (content, rest) = rest.split_at_mut(content_length);
        let (tag, _) = rest.split_at_mut(TAG_SIZE);

        assert_eq!(
            LENGTH_SIZE + FLAG_SIZE + NONCE_SIZE + hello.len() + TAG_SIZE,
            len
        );
        assert_eq!(u16::try_from(hello.len())?.to_be_bytes(), length);
        assert_eq!([1], encrypt);

        let nonce = nonce.try_into()?;
        let tag = tag.try_into()?;

        cipher.decrypt(content, &nonce, &tag)?;

        assert_eq!(hello, content);

        Ok(())
    }

    #[test]
    fn writing_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let input = [
            0, 5, 1, 179, 28, 101, 187, 68, 71, 151, 166, 11, 210, 114, 41, 213, 134, 31, 3, 158,
            84, 116, 75, 159, 94, 135, 120, 164, 81, 79, 119, 171, 55, 70, 42, 37,
        ];

        let cipher = Cipher::new(&[b'a'; 32]);

        let mut buffer = [0; 1000];

        let message = Message::new_encrypted(&mut buffer, &cipher);

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
        let message = &buf.message;

        assert_eq!(input.len(), len);
        assert_eq!(input[..2], message.length);
        assert_eq!([1], message.get_encryption_bit());

        let n = buf.consume_encrypted(&cipher).map_err(Error::ignore)?;
        assert_eq!(b"Hello", &buffer[..n]);

        Ok(())
    }
}
