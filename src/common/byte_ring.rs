use bytes::buf::Buf;
use bytes::buf::BufMut;

pub struct ByteRing {
    // tail is the next byte to read
    // head is the next byte to write
    // head == tail -> buffer is empty
    head: usize,
    tail: usize,
    buf: Vec<u8>,
}

impl ByteRing {
    pub fn with_capacity(n: usize) -> ByteRing {
        ByteRing { head: 0, tail: 0, buf: vec![0; n] }
    }
}

impl Buf for ByteRing {
    fn remaining(&self) -> usize {
        if self.head >= self.tail {
            // ---TxxxxxxxxxxH------
            self.head - self.tail
        } else {
            // xxxH----------Txxxxxx
            self.buf.len() - self.tail + self.head
        }
    }

    fn bytes(&self) -> &[u8] {
        if self.head >= self.tail {
            // ---TxxxxxxxxxxH------
            &self.buf[self.tail..self.head]
        } else {
            // xxxH----------Txxxxxx
            &self.buf[self.tail..]
        }
    }

    fn advance(&mut self, cnt: usize) {
        let next = (self.tail + cnt) % self.buf.len();

        // TODO: replace these asserts with actual panics?

        if self.head >= self.tail {
            // ---TxxxxxxxxxxH------
            // --------TxxxxxH------
            assert!(next <= self.head);
        } else {
            // xxxH----------Txxxxxx
            // xxxH---------------Tx
            // -TxH----------------- (also possible)
            assert!(next > self.tail || next <= self.head);
        }

        debug!("advance {}..{} + {} -> {}..{}", self.tail, self.head, cnt, next, self.head);
        self.tail = next;
    }
}

impl BufMut for ByteRing {
    fn remaining_mut(&self) -> usize {
        if self.head >= self.tail {
            // ---TxxxxxxxxxxH------
            self.buf.len() - self.head + self.tail - 1
        } else {
            // xxxH----------Txxxxxx
            self.tail - self.head - 1
        }
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        let next = (self.head + cnt) % self.buf.len();

        // TODO: replace these asserts with actual panics?

        if self.head >= self.tail {
            // ---TxxxxxxxxxxH------
            // ---TxxxxxxxxxxxxxxxH-
            // xH-Txxxxxxxxxxxxxxxxx (also possible)
            assert!(next > self.head || next < self.tail);
            // strict '<' allows one byte between head and tail
        } else {
            // xxxH----------Txxxxxx
            // xxxxxxxxH-----Txxxxxx
            assert!(next < self.tail);
            // strict '<' allows one byte between head and tail
        }

        debug!("advance_mut {}..{} + {} -> {}..{}", self.tail, self.head, cnt, self.tail, next);
        self.head = next;
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        if self.head >= self.tail {
            // ---TxxxxxxxxxxH------
            &mut self.buf[self.head..]
        } else {
            // xxxH----------Txxxxxx
            &mut self.buf[self.head..self.tail]
        }
    }
}
