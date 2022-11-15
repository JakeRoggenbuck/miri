use rustc_data_structures::fx::FxHashSet;

use crate::*;

pub trait VisitTags {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag));
}

impl<T: VisitTags> VisitTags for Option<T> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        if let Some(x) = self {
            x.visit_tags(visit);
        }
    }
}

impl<T: VisitTags> VisitTags for std::cell::RefCell<T> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        self.borrow().visit_tags(visit)
    }
}

impl VisitTags for SbTag {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        visit(*self)
    }
}

impl VisitTags for Provenance {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        if let Provenance::Concrete { sb, .. } = self {
            visit(*sb);
        }
    }
}

impl VisitTags for Pointer<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        let (prov, _offset) = self.into_parts();
        prov.visit_tags(visit);
    }
}

impl VisitTags for Pointer<Option<Provenance>> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        let (prov, _offset) = self.into_parts();
        prov.visit_tags(visit);
    }
}

impl VisitTags for Scalar<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        match self {
            Scalar::Ptr(ptr, _) => ptr.visit_tags(visit),
            Scalar::Int(_) => (),
        }
    }
}

impl VisitTags for Immediate<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        match self {
            Immediate::Scalar(s) => {
                s.visit_tags(visit);
            }
            Immediate::ScalarPair(s1, s2) => {
                s1.visit_tags(visit);
                s2.visit_tags(visit);
            }
            Immediate::Uninit => {}
        }
    }
}

impl VisitTags for MemPlaceMeta<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        match self {
            MemPlaceMeta::Meta(m) => m.visit_tags(visit),
            MemPlaceMeta::None => {}
        }
    }
}

impl VisitTags for MemPlace<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        let MemPlace { ptr, meta } = self;
        ptr.visit_tags(visit);
        meta.visit_tags(visit);
    }
}

impl VisitTags for MPlaceTy<'_, Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        (**self).visit_tags(visit)
    }
}

impl VisitTags for Place<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        match self {
            Place::Ptr(p) => p.visit_tags(visit),
            Place::Local { .. } => {
                // Will be visited as part of the stack frame.
            }
        }
    }
}

impl VisitTags for PlaceTy<'_, Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        (**self).visit_tags(visit)
    }
}

impl VisitTags for Operand<Provenance> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        match self {
            Operand::Immediate(imm) => {
                imm.visit_tags(visit);
            }
            Operand::Indirect(p) => {
                p.visit_tags(visit);
            }
        }
    }
}

impl VisitTags for Allocation<Provenance, AllocExtra> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        for prov in self.provenance().provenances() {
            prov.visit_tags(visit);
        }

        self.extra.visit_tags(visit);
    }
}

impl VisitTags for crate::MiriInterpCx<'_, '_> {
    fn visit_tags(&self, visit: &mut dyn FnMut(SbTag)) {
        // Memory.
        self.memory.alloc_map().iter(|it| {
            for (_id, (_kind, alloc)) in it {
                alloc.visit_tags(visit);
            }
        });

        // And all the other machine values.
        self.machine.visit_tags(visit);
    }
}

impl<'mir, 'tcx: 'mir> EvalContextExt<'mir, 'tcx> for crate::MiriInterpCx<'mir, 'tcx> {}
pub trait EvalContextExt<'mir, 'tcx: 'mir>: MiriInterpCxExt<'mir, 'tcx> {
    fn garbage_collect_tags(&mut self) -> InterpResult<'tcx> {
        let this = self.eval_context_mut();
        // No reason to do anything at all if stacked borrows is off.
        if this.machine.stacked_borrows.is_none() {
            return Ok(());
        }

        let mut tags = FxHashSet::default();
        this.visit_tags(&mut |tag| {
            tags.insert(tag);
        });
        self.remove_unreachable_tags(tags);

        Ok(())
    }

    fn remove_unreachable_tags(&mut self, tags: FxHashSet<SbTag>) {
        let this = self.eval_context_mut();
        this.memory.alloc_map().iter(|it| {
            for (_id, (_kind, alloc)) in it {
                alloc
                    .extra
                    .stacked_borrows
                    .as_ref()
                    .unwrap()
                    .borrow_mut()
                    .remove_unreachable_tags(&tags);
            }
        });
    }
}
