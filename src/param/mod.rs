// parse a parameter block

pub struct ParamBlockSchema {
    pub binary: usize,
    pub varlen: usize,
    pub stored: usize,
    pub temp: usize,
    pub integer: usize,
}

pub struct StoredBlob {
    pub data: Vec<u8>,
}
pub struct TempBlob {
    pub data: Vec<u8>,
}

pub struct ParamBlock {
    pub integer: Box<[u64]>,
    pub stored: Box<[StoredBlob]>,
    pub temp: Box<[TempBlob]>,
    pub binary: Box<[u8]>,
    pub varlen: Box<[[u8]]>,
}

pub fn parse_complete(input: &[u8], schema: &ParamBlockSchema) -> Result<ParamBlock, String> {
    let mut integer = vec![0; schema.integer];
    let mut stored = vec![StoredBlob::default(); schema.stored];
    let mut temp = vec![TempBlob::default(); schema.temp];
    let mut binary = vec![0; schema.binary];
    let mut varlen = vec![vec![0; 0]; schema.varlen];

    // parse the input buffer into the param block
    // ...

    Ok(ParamBlock {
        integer: integer.into_boxed_slice(),
        stored: stored.into_boxed_slice(),
        temp: temp.into_boxed_slice(),
        binary: binary.into_boxed_slice(),
        varlen: varlen.into_boxed_slice(),
    })
}

async fn parse<R: AsyncRead + Unpin>(
    reader: &mut R,
    schema: &ParamBlockSchema,
) -> Result<ParamBlock, String> {
    let mut integer = vec![0; schema.integer];
    let mut stored = vec![StoredBlob::default(); schema.stored];
    let mut temp = vec![TempBlob::default(); schema.temp];
    let mut binary = vec![0; schema.binary];
    let mut varlen = vec![vec![0; 0]; schema.varlen];

    // parse the input buffer into the param block
    // ...

    Ok(ParamBlock {
        integer: integer.into_boxed_slice(),
        stored: stored.into_boxed_slice(),
        temp: temp.into_boxed_slice(),
        binary: binary.into_boxed_slice(),
        varlen: varlen.into_boxed_slice(),
    })
}
