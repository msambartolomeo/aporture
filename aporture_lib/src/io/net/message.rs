use crate::crypto::cipher::Cipher;
use bytes::{Buf, BufMut};

const LENGHT_SIZE: usize = 2;
const FLAG_SIZE: usize = 1;
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;

#[derive(Debug)]
pub struct Message<'a> {
    length: [u8; LENGHT_SIZE],
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
    fn plain() -> Self {
        EncryptedContent::Plain { bit: [0] }
    }

    fn encrypted(nonce: [u8; NONCE_SIZE], tag: [u8; TAG_SIZE]) -> Self {
        EncryptedContent::Encrypted {
            bit: [1],
            nonce,
            tag,
        }
    }
}

impl<'a> Message<'a> {
    #[allow(clippy::cast_possible_truncation)] // As truncation is checked explicitly
    pub fn new(content: &'a mut [u8], cipher: Option<&Cipher>) -> Self {
        let length = content.len();

        assert!(
            length < u16::MAX.into(),
            "Message Payload must be smaller than u16::MAX"
        );

        let length = (length as u16).to_be_bytes();

        match cipher {
            Some(cipher) => {
                let (nonce, tag) = cipher.encrypt(content);

                Self {
                    length,
                    encrypted: EncryptedContent::encrypted(nonce, tag),
                    content,
                }
            }
            None => Self {
                length,
                encrypted: EncryptedContent::plain(),
                content: content,
            },
        }
    }

    #[must_use]
    pub fn into_buf(self) -> MessageBuffer<'a> {
        MessageBuffer::new(self)
    }

    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        match self.encrypted {
            EncryptedContent::Plain { .. } => false,
            EncryptedContent::Encrypted { .. } => true,
        }
    }

    pub fn get_encryption_bit(&self) -> [u8; 1] {
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
    error: bool,
}

impl<'a> MessageBuffer<'a> {
    pub fn new(message: Message<'a>) -> Self {
        Self {
            message,
            state: State::Length,
            cursor: 0,
            error: false,
        }
    }

    // TODO: ERROR
    pub fn consume(self, cipher: Option<&Cipher>) -> Result<&'a [u8], ()> {
        if self.error {
            return Err(());
        }

        let length = self.message.length();
        let content = &mut self.message.content[..length];

        match (&self.message.encrypted, cipher) {
            (EncryptedContent::Encrypted { nonce, tag, .. }, Some(c)) => {
                c.decrypt(content, nonce, tag).unwrap();
            }
            (EncryptedContent::Plain { .. }, None) => {}
            _ => return Err(()),
        }

        Ok(content)
    }

    fn total_remaining(&self, state: State) -> usize {
        match state {
            State::Length => LENGHT_SIZE + FLAG_SIZE,
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

impl<'a> Buf for MessageBuffer<'a> {
    fn remaining(&self) -> usize {
        let total_remaining = self.total_remaining(self.state);

        total_remaining - self.cursor
    }

    fn chunk(&self) -> &[u8] {
        let slice: &[u8] = match &self.message.encrypted {
            EncryptedContent::Plain { bit } => match self.state {
                State::Length => &self.message.length,
                State::Encrypt => bit,
                State::Content => &self.message.content,
                _ => unreachable!("Plain cannot have other states"),
            },
            EncryptedContent::Encrypted { nonce, tag, bit } => match self.state {
                State::Length => &self.message.length,
                State::Encrypt => bit,
                State::Content => &self.message.content,
                State::Nonce => nonce,
                State::Tag => tag,
            },
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

unsafe impl<'a> BufMut for MessageBuffer<'a> {
    fn remaining_mut(&self) -> usize {
        if self.error {
            return 0;
        }

        let total_remaining = self.total_remaining(self.state);

        total_remaining - self.cursor
    }

    unsafe fn advance_mut(&mut self, mut cnt: usize) {
        assert!(cnt <= self.remaining_mut());

        loop {
            let chunk_lenght = self.chunk_mut().len();

            if cnt < chunk_lenght {
                self.cursor += cnt;
                break;
            }

            cnt -= chunk_lenght;
            self.cursor = 0;

            self.state = match self.state {
                State::Length => State::Encrypt,
                State::Encrypt => {
                    let content_length = self.message.length();
                    let available_length = self.message.content.len();
                    if available_length < content_length {
                        self.error = true;
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
                            self.error = true;
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
                State::Content => &mut self.message.content,
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

    use crate::parser::Parser;
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

        let mut serialized = hello.serialize_to();

        let message = Message::new(&mut serialized, None);

        let content = hello.serialize_to();

        assert_eq!(false, message.is_encrypted());
        assert_eq!(content.len(), message.length());
        assert_eq!(content, *message.content);
    }

    #[test]
    fn reading() -> Result<(), Box<dyn std::error::Error>> {
        let input = PairKind::Sender;

        let mut serialized = input.serialize_to();

        let message = Message::new(&mut serialized, None);

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

        let mut buf = [0; 1000];

        let message = Message::new(&mut buf, None);

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
        assert_eq!(3u16.to_be_bytes(), message.length);
        assert_eq!([0], message.get_encryption_bit());

        let output = buf.consume(None).unwrap();

        assert_eq!(&input[3..], output);
        assert_eq!(PairKind::Sender.serialize_to(), output);

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

        let mut serialized = hello.serialize_to();

        let message = Message::new(&mut serialized, Some(&cipher));

        let content = hello.serialize_to();

        assert_eq!(true, message.is_encrypted());
        assert_eq!(content.len(), message.length());
    }

    #[test]
    fn reading_encrypted() -> Result<(), Box<dyn std::error::Error>> {
        let input = PairKind::Sender;

        let cipher = Cipher::new(&[b'a'; 32]);

        let mut serialized = input.serialize_to();

        let message = Message::new(&mut serialized, Some(&cipher));

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

        let mut buf = [0; 1000];

        let message = Message::new(&mut buf, Some(&cipher));

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
        assert_eq!(3u16.to_be_bytes(), message.length);
        assert_eq!([1], message.get_encryption_bit());

        dbg!(input);
        dbg!(&buf);

        let output = buf.consume(Some(&cipher)).unwrap();
        assert_eq!(PairKind::Sender.serialize_to(), output);

        Ok(())
    }
}
