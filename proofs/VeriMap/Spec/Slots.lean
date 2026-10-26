-- Human-authored vocabulary for the kernel's per-object revision rules
-- (docs/verification.md rows 2–5). The slot type is the one Aeneas
-- generates from `verimap_kernel::Slot`; these definitions only name what
-- the properties talk about.
module

public import VeriMap.Code.Types

@[expose] public section

namespace VeriMap.Spec

open VeriMap

/-- The revision a slot records. A vacant slot counts as 0, below every
committed sequence (sequences start at 1). -/
def slotRevision : Slot → Nat
  | .Vacant => 0
  | .Live r => r.val
  | .Deleted r => r.val

/-- The slot holds a deletion tombstone. -/
def IsTombstone : Slot → Prop
  | .Deleted _ => True
  | _ => False

end VeriMap.Spec
