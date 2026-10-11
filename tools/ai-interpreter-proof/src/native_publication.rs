//! Prepared changed-row publication into an exclusively borrowed application mesh.
//! Comparison is O(base); payload copies are O(changed). JSON Clone and the live
//! application/history allocation are not bounded by this logical lease.
use crate::owner_budget::{Kind, Lease, OwnerBudget};
use ingetrazo_core::mesh::{Mesh, MeshEdge, MeshFace, MeshVertex, PreparedSpatialIndex};
use std::mem::size_of;
#[derive(Clone, Copy, Debug, Default, serde::Serialize, PartialEq, Eq)]
pub struct PublicationMetrics {
    pub compared_rows: usize,
    pub vertex_rows: usize,
    pub edge_rows: usize,
    pub face_rows: usize,
    pub index_vertices: usize,
    pub payload_bytes: usize,
}
struct Change<T> {
    index: usize,
    before: Option<T>,
    after: Option<T>,
}
trait Row: Clone + PartialEq {
    fn bytes(&self) -> Result<usize, String>;
}
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or("Publication byte extent overflow".into())
}
fn mul(a: usize, b: usize) -> Result<usize, String> {
    a.checked_mul(b)
        .ok_or("Publication byte extent overflow".into())
}
impl Row for MeshVertex {
    fn bytes(&self) -> Result<usize, String> {
        add(
            size_of::<Self>(),
            mul(self.edges.len(), size_of::<usize>())?,
        )
    }
}
impl Row for MeshEdge {
    fn bytes(&self) -> Result<usize, String> {
        let n = add(
            size_of::<Self>(),
            mul(self.faces.len(), size_of::<usize>())?,
        )?;
        add(
            add(n, self.layer.as_ref().map_or(0, String::len))?,
            crate::face_owner::attribute_payload(&self.attrs)?,
        )
    }
}
impl Row for MeshFace {
    fn bytes(&self) -> Result<usize, String> {
        let mut n = add(
            size_of::<Self>(),
            mul(self.vertices.len(), size_of::<usize>())?,
        )?;
        n = add(n, mul(self.holes.len(), size_of::<Vec<usize>>())?)?;
        for h in &self.holes {
            n = add(n, mul(h.len(), size_of::<usize>())?)?;
        }
        add(
            add(n, self.layer.as_ref().map_or(0, String::len))?,
            crate::face_owner::attribute_payload(&self.attrs)?,
        )
    }
}
fn extent<T: Row>(before: &[T], after: &[T]) -> Result<(usize, usize), String> {
    let mut count = 0;
    let mut bytes = 0;
    for i in 0..before.len().max(after.len()) {
        let a = before.get(i);
        let b = after.get(i);
        if a != b {
            count += 1;
            bytes = add(bytes, size_of::<Change<T>>())?;
            if let Some(a) = a {
                bytes = add(bytes, a.bytes()?)?;
            }
            if let Some(b) = b {
                bytes = add(bytes, b.bytes()?)?;
            }
        }
    }
    Ok((count, bytes))
}
fn changes<T: Row>(before: &[T], after: &[T], count: usize) -> Result<Vec<Change<T>>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| "Publication row allocation refused")?;
    for i in 0..before.len().max(after.len()) {
        if before.get(i) != after.get(i) {
            out.push(Change {
                index: i,
                before: before.get(i).cloned(),
                after: after.get(i).cloned(),
            });
        }
    }
    Ok(out)
}
fn validate<T: Row>(rows: &[T], changes: &[Change<T>]) -> Result<(), String> {
    for c in changes {
        if rows.get(c.index) != c.before.as_ref() {
            return Err("Stale application publication row".into());
        }
    }
    Ok(())
}
fn reserve<T>(rows: &mut Vec<T>, desired: usize) -> Result<(), String> {
    rows.try_reserve(desired.saturating_sub(rows.len()))
        .map_err(|_| "Application row capacity refused".into())
}
fn publish<T>(rows: &mut Vec<T>, desired: usize, changes: Vec<Change<T>>) {
    rows.truncate(desired);
    for c in changes {
        if let Some(after) = c.after {
            if c.index < rows.len() {
                rows[c.index] = after;
            } else {
                assert_eq!(c.index, rows.len(), "Prepared publication row order");
                rows.push(after);
            }
        }
    }
    assert_eq!(rows.len(), desired);
}
/// Opaque transaction ticket. All candidates remain private until prepare succeeds.
pub struct NativePublication {
    before: (usize, usize, usize),
    after: (usize, usize, usize),
    vertices: Vec<Change<MeshVertex>>,
    edges: Vec<Change<MeshEdge>>,
    faces: Vec<Change<MeshFace>>,
    index: Option<(PreparedSpatialIndex, Lease)>,
    _charge: Lease,
    metrics: PublicationMetrics,
}
impl NativePublication {
    pub(crate) fn between(
        before: &Mesh,
        after: &Mesh,
        budget: &OwnerBudget,
    ) -> Result<Self, String> {
        let (v, vb) = extent(&before.vertices, &after.vertices)?;
        let (e, eb) = extent(&before.edges, &after.edges)?;
        let (f, fb) = extent(&before.faces, &after.faces)?;
        let bytes = add(add(vb, eb)?, fb)?;
        let charge = budget.reserve(Kind::Preparation, bytes)?;
        let index = before.vertices.len() != after.vertices.len()
            || before
                .vertices
                .iter()
                .zip(&after.vertices)
                .any(|(a, b)| a.position != b.position);
        let index = if index {
            let table = after
                .vertices
                .len()
                .max(1)
                .checked_next_power_of_two()
                .and_then(|n| n.checked_mul(4))
                .ok_or("Publication index extent overflow")?;
            let bytes = add(
                mul(after.vertices.len(), size_of::<glam::DVec3>() * 2)?,
                mul(table, 32)?,
            )?;
            let charge = budget.reserve(Kind::Preparation, bytes)?;
            let mut positions = Vec::new();
            positions
                .try_reserve_exact(after.vertices.len())
                .map_err(|_| "Publication coordinates refused")?;
            positions.extend(after.vertices.iter().map(|v| v.position));
            Some((
                Mesh::prepare_spatial_index(&positions).map_err(str::to_owned)?,
                charge,
            ))
        } else {
            None
        };
        let metrics = PublicationMetrics {
            compared_rows: add(
                add(
                    before.vertices.len().max(after.vertices.len()),
                    before.edges.len().max(after.edges.len()),
                )?,
                before.faces.len().max(after.faces.len()),
            )?,
            vertex_rows: v,
            edge_rows: e,
            face_rows: f,
            index_vertices: if index.is_some() {
                after.vertices.len()
            } else {
                0
            },
            payload_bytes: bytes,
        };
        Ok(Self {
            before: (
                before.vertices.len(),
                before.edges.len(),
                before.faces.len(),
            ),
            after: (after.vertices.len(), after.edges.len(), after.faces.len()),
            vertices: changes(&before.vertices, &after.vertices, v)?,
            edges: changes(&before.edges, &after.edges, e)?,
            faces: changes(&before.faces, &after.faces, f)?,
            index,
            _charge: charge,
            metrics,
        })
    }
    pub fn metrics(&self) -> PublicationMetrics {
        self.metrics
    }
    /// Exclusive borrow survives through allocation-free publish. Refusal can change
    /// vector capacity but never logical rows or history. No script runs in this phase.
    pub fn prepare(self, target: &mut Mesh) -> Result<PreparedPublication<'_>, String> {
        if (
            target.vertices.len(),
            target.edges.len(),
            target.faces.len(),
        ) != self.before
        {
            return Err("Stale application publication extent".into());
        }
        validate(&target.vertices, &self.vertices)?;
        validate(&target.edges, &self.edges)?;
        validate(&target.faces, &self.faces)?;
        if let Some((index, _)) = &self.index {
            // A caller may have changed an otherwise-unmodified coordinate after
            // ticket creation. Validate the complete virtual ordered positions
            // before any write; the index install cannot then discover staleness.
            let mut changed = self.vertices.iter().peekable();
            for (i, &position) in index.positions().iter().enumerate() {
                let actual = if changed.peek().is_some_and(|c| c.index == i) {
                    changed
                        .next()
                        .unwrap()
                        .after
                        .as_ref()
                        .ok_or("Missing prepared vertex")?
                        .position
                } else {
                    target
                        .vertices
                        .get(i)
                        .ok_or("Missing unchanged vertex")?
                        .position
                };
                if actual != position {
                    return Err("Stale unchanged application coordinate".into());
                }
            }
        }
        reserve(&mut target.vertices, self.after.0)?;
        reserve(&mut target.edges, self.after.1)?;
        reserve(&mut target.faces, self.after.2)?;
        Ok(PreparedPublication {
            target,
            publication: self,
        })
    }
}
pub struct PreparedPublication<'a> {
    target: &'a mut Mesh,
    publication: NativePublication,
}
impl PreparedPublication<'_> {
    pub fn publish(self) -> PublicationMetrics {
        let Self {
            target,
            publication,
        } = self;
        let NativePublication {
            after,
            vertices,
            edges,
            faces,
            metrics,
            index,
            _charge: _payload_charge,
            ..
        } = publication;
        publish(&mut target.vertices, after.0, vertices);
        publish(&mut target.edges, after.1, edges);
        publish(&mut target.faces, after.2, faces);
        // Exclusive target ownership and the precomputed ordered positions prove this
        // cannot fail. No fallible allocation remains after the first logical write.
        if let Some((index, _charge)) = index {
            target
                .install_prepared_spatial_index(index)
                .expect("Exclusive prepared application positions");
        }
        metrics
    }
}
