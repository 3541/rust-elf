use crate::parse::{parse_u32_at, Class, Endian, ParseAt, ParseError, U32Table};
use crate::string_table::StringTable;
use crate::symbol::{Symbol, SymbolTable};

/// Header at the start of SysV Hash Table sections of type [SHT_HASH](crate::gabi::SHT_HASH).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SysVHashHeader {
    pub nbucket: u32,
    pub nchain: u32,
}

impl ParseAt for SysVHashHeader {
    fn parse_at(
        endian: Endian,
        _class: Class,
        offset: &mut usize,
        data: &[u8],
    ) -> Result<Self, ParseError> {
        Ok(SysVHashHeader {
            nbucket: parse_u32_at(endian, offset, data)?,
            nchain: parse_u32_at(endian, offset, data)?,
        })
    }

    #[inline]
    fn size_for(_class: Class) -> usize {
        core::mem::size_of::<u32>() + core::mem::size_of::<u32>()
    }
}

/// Calculate the SysV hash value for a given symbol name.
pub fn sysv_hash(name: &[u8]) -> u32 {
    let mut hash = 0u32;
    for byte in name {
        hash = hash.wrapping_mul(16).wrapping_add(*byte as u32);
        hash ^= (hash >> 24) & 0xf0;
    }
    hash & 0xfffffff
}

#[derive(Debug)]
pub struct SysVHashTable<'data> {
    buckets: U32Table<'data>,
    chains: U32Table<'data>,
}

/// This constructs a lazy-parsing type that keeps a reference to the provided data
/// bytes from which it lazily parses and interprets its contents.
impl<'data> SysVHashTable<'data> {
    /// Construct a SysVHashTable from given bytes. Keeps a reference to the data for lazy parsing.
    pub fn new(endian: Endian, class: Class, data: &'data [u8]) -> Result<Self, ParseError> {
        let mut offset = 0;
        let hdr = SysVHashHeader::parse_at(endian, class, &mut offset, data)?;

        let bucket_size = hdr.nbucket as usize * u32::size_for(class);
        let bucket_buf = data
            .get(offset..offset + bucket_size)
            .ok_or(ParseError::BadOffset(offset as u64))?;
        let buckets = U32Table::new(endian, class, bucket_buf);
        offset += bucket_size;

        let chain_size = hdr.nchain as usize * u32::size_for(class);
        let chain_buf = data
            .get(offset..offset + chain_size)
            .ok_or(ParseError::BadOffset(offset as u64))?;
        let chains = U32Table::new(endian, class, chain_buf);

        Ok(SysVHashTable { buckets, chains })
    }

    /// Use the hash table to find the symbol table entry with the given name and hash.
    pub fn find(
        &self,
        name: &[u8],
        hash: u32,
        symtab: &SymbolTable<'data>,
        strtab: &StringTable<'data>,
    ) -> Result<Option<(usize, Symbol)>, ParseError> {
        let start = (hash as usize) % self.buckets.len();
        let mut index = self.buckets.get(start)? as usize;

        // Bound the number of chain lookups by the chain size so we don't loop forever
        let mut i = 0;
        while index != 0 && i < self.chains.len() {
            let symbol = symtab.get(index)?;
            if strtab.get_raw(symbol.st_name as usize)? == name {
                return Ok(Some((index, symbol)));
            }

            index = self.chains.get(index)? as usize;
            i += 1;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod sysv_parse_tests {
    use super::*;
    use crate::parse::{test_parse_for, test_parse_fuzz_too_short};

    #[test]
    fn parse_sysvhdr32_lsb() {
        test_parse_for(
            Endian::Little,
            Class::ELF32,
            SysVHashHeader {
                nbucket: 0x03020100,
                nchain: 0x07060504,
            },
        );
    }

    #[test]
    fn parse_sysvhdr32_msb() {
        test_parse_for(
            Endian::Big,
            Class::ELF32,
            SysVHashHeader {
                nbucket: 0x00010203,
                nchain: 0x04050607,
            },
        );
    }

    #[test]
    fn parse_sysvhdr64_lsb() {
        test_parse_for(
            Endian::Little,
            Class::ELF64,
            SysVHashHeader {
                nbucket: 0x03020100,
                nchain: 0x07060504,
            },
        );
    }

    #[test]
    fn parse_sysvhdr64_msb() {
        test_parse_for(
            Endian::Big,
            Class::ELF64,
            SysVHashHeader {
                nbucket: 0x00010203,
                nchain: 0x04050607,
            },
        );
    }

    #[test]
    fn parse_sysvhdr32_lsb_fuzz_too_short() {
        test_parse_fuzz_too_short::<SysVHashHeader>(Endian::Little, Class::ELF32);
    }

    #[test]
    fn parse_sysvhdr32_msb_fuzz_too_short() {
        test_parse_fuzz_too_short::<SysVHashHeader>(Endian::Big, Class::ELF32);
    }

    #[test]
    fn parse_sysvhdr64_lsb_fuzz_too_short() {
        test_parse_fuzz_too_short::<SysVHashHeader>(Endian::Little, Class::ELF64);
    }

    #[test]
    fn parse_sysvhdr64_msb_fuzz_too_short() {
        test_parse_fuzz_too_short::<SysVHashHeader>(Endian::Big, Class::ELF64);
    }
}
