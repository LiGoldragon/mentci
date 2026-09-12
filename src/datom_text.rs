//! Canonical datom text in and out.
//!
//! Every text the daemon reads or writes in the datom dialect passes through
//! here, so the budgets that bound a read are stated once and the projection
//! out is the exact reverse of the projection in.

use datom_codec::{
    Actualizing, Budget, Compositional, Datom, Datomizable, Path as DatomPath, Potential,
};
use protos::{Protosizable, ReaderBudget, Textualizable};

use crate::Error;

/// Bounds for reading one datom value. The values this daemon reads are small,
/// shallow records; these refuse a runaway text before it can cost anything.
const COMPOSITION_NODES: i64 = 65_536;
const READER_NODES: usize = 1_048_576;
const COMPOSITION_DEPTH: i64 = 64;

/// Project a datom-bearing value to its canonical datom text. Cannot fault.
pub fn textualize<Value>(value: &Value) -> String
where
    Value: Datomizable<Output = Datom>,
{
    value.datomize(DatomPath::new()).protosize().textualize()
}

/// Read one datom value of the expected type out of text.
pub fn actualize<Value>(source: &str) -> crate::Result<Value>
where
    Value: Compositional,
{
    Potential::<Value>::from(source.to_owned())
        .actualize(&mut budget())
        .map_err(|error| Error::Datom(textualize(&error)))
}

fn budget() -> Budget {
    Budget {
        remaining: COMPOSITION_NODES,
        reader: ReaderBudget {
            remaining: READER_NODES,
        },
        depth: 0,
        maximum_depth: COMPOSITION_DEPTH,
    }
}
