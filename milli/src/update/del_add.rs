use obkv::Key;

pub type KvWriterDelAdd<W> = obkv::KvWriter<W, DelAdd>;
pub type KvReaderDelAdd<'a> = obkv::KvReader<'a, DelAdd>;

/// DelAdd defines the new value to add in the database and old value to delete from the database.
///
/// Its used in an OBKV to be serialized in grenad files.
#[repr(u8)]
#[derive(Clone, Copy, PartialOrd, PartialEq, Debug)]
pub enum DelAdd {
    Deletion = 0,
    Addition = 1,
}

impl Key for DelAdd {
    const BYTES_SIZE: usize = std::mem::size_of::<DelAdd>();
    type BYTES = [u8; Self::BYTES_SIZE];

    fn to_be_bytes(&self) -> Self::BYTES {
        u8::to_be_bytes(*self as u8)
    }

    fn from_be_bytes(array: Self::BYTES) -> Self {
        match u8::from_be_bytes(array) {
            0 => Self::Deletion,
            1 => Self::Addition,
            otherwise => unreachable!("DelAdd has only 2 variants, unknown variant: {}", otherwise),
        }
    }
}

/// Creates a Kv<K, Kv<DelAdd, value>> from Kv<K, value>
///
/// if deletion is `true`, the value will be inserted behind a DelAdd::Deletion key.
/// if addition is `true`, the value will be inserted behind a DelAdd::Addition key.
/// if both deletion and addition are `true, the value will be inserted in both keys.
pub fn into_del_add_obkv<K: obkv::Key + PartialOrd>(
    reader: obkv::KvReader<K>,
    deletion: bool,
    addition: bool,
    buffer: &mut Vec<u8>,
) -> Result<(), std::io::Error> {
    let mut writer = obkv::KvWriter::new(buffer);
    let mut value_buffer = Vec::new();
    for (key, value) in reader.iter() {
        value_buffer.clear();
        let mut value_writer = KvWriterDelAdd::new(&mut value_buffer);
        if deletion {
            value_writer.insert(DelAdd::Deletion, value)?;
        }
        if addition {
            value_writer.insert(DelAdd::Addition, value)?;
        }
        value_writer.finish()?;
        writer.insert(key, &value_buffer)?;
    }

    writer.finish()
}

/// Creates a Kv<K, Kv<DelAdd, value>> from two Kv<K, value>
///
/// putting each deletion obkv's keys under an DelAdd::Deletion
/// and putting each addition obkv's keys under an DelAdd::Addition
pub fn del_add_from_two_obkvs<K: obkv::Key + PartialOrd + Ord>(
    deletion: obkv::KvReader<K>,
    addition: obkv::KvReader<K>,
    buffer: &mut Vec<u8>,
) -> Result<(), std::io::Error> {
    use itertools::merge_join_by;
    use itertools::EitherOrBoth::{Both, Left, Right};

    let mut writer = obkv::KvWriter::new(buffer);
    let mut value_buffer = Vec::new();

    for eob in merge_join_by(deletion.iter(), addition.iter(), |(b, _), (u, _)| b.cmp(u)) {
        value_buffer.clear();
        match eob {
            Left((k, v)) => {
                let mut value_writer = KvWriterDelAdd::new(&mut value_buffer);
                value_writer.insert(DelAdd::Deletion, v).unwrap();
                writer.insert(k, value_writer.into_inner()?).unwrap();
            }
            Right((k, v)) => {
                let mut value_writer = KvWriterDelAdd::new(&mut value_buffer);
                value_writer.insert(DelAdd::Addition, v).unwrap();
                writer.insert(k, value_writer.into_inner()?).unwrap();
            }
            Both((k, deletion), (_, addition)) => {
                let mut value_writer = KvWriterDelAdd::new(&mut value_buffer);
                value_writer.insert(DelAdd::Deletion, deletion).unwrap();
                value_writer.insert(DelAdd::Addition, addition).unwrap();
                writer.insert(k, value_writer.into_inner()?).unwrap();
            }
        }
    }

    writer.finish()
}
