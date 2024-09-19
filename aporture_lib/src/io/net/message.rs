use crate::{crypto::cipher::Cipher, parser::Parser};
use bytes::Buf;

#[derive(Debug, Clone)]
pub enum Message {
    Plain {
        length: [u8; 2],
        content: Vec<u8>,
    },
    Encrypted {
        length: [u8; 2],
        nonce: [u8; 12],
        content: Vec<u8>,
        tag: [u8; 16],
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

                Self::Encrypted {
                    length,
                    nonce,
                    content,
                    tag,
                }
            }
            None => Self::Plain { length, content },
        }
    }

    pub fn into_buf(self) -> MessageBuffer {
        self.into()
    }

    pub fn is_encrypted(&self) -> bool {
        match self {
            Message::Plain { .. } => false,
            Message::Encrypted { .. } => true,
        }
    }

    fn content(&self) -> &[u8] {
        match self {
            Message::Plain { content, .. } => content,
            Message::Encrypted { content, .. } => content,
        }
    }

    fn slice_from_state(&self, state: State) -> &[u8] {
        match &self {
            Message::Plain { length, content } => match state {
                State::Length => length,
                State::Encrypt => &[0],
                State::Content => content,
                _ => unreachable!("Plain cannot have other states"),
            },
            Message::Encrypted {
                length,
                nonce,
                content,
                tag,
            } => match state {
                State::Length => length,
                State::Encrypt => &[1],
                State::Nonce => nonce,
                State::Content => content,
                State::Tag => tag,
            },
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

#[derive(Debug, Clone)]
pub struct MessageBuffer {
    message: Message,
    state: State,
    cursor: usize,
}

impl From<Message> for MessageBuffer {
    fn from(message: Message) -> Self {
        MessageBuffer {
            message,
            state: State::Length,
            cursor: 0,
        }
    }
}

impl Buf for MessageBuffer {
    fn remaining(&self) -> usize {
        let content_length = self.message.content().len();

        let remaining = if self.message.is_encrypted() {
            match self.state {
                State::Length => 1 + content_length,
                State::Encrypt => content_length,
                State::Content => 0,
                _ => unreachable!("Not possible in message plain"),
            }
        } else {
            match self.state {
                State::Length => 1 + 12 + content_length + 16,
                State::Encrypt => 12 + content_length + 16,
                State::Nonce => content_length + 16,
                State::Content => 16,
                State::Tag => 0,
            }
        };

        self.chunk().len() + remaining
    }

    fn chunk(&self) -> &[u8] {
        &self.message.slice_from_state(self.state)[self.cursor..]
    }

    fn advance(&mut self, mut cnt: usize) {
        assert!(cnt < self.remaining());

        loop {
            let chunk_lenght = self.chunk().len();

            if cnt < chunk_lenght {
                self.cursor += cnt;
                break;
            } else {
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
}
