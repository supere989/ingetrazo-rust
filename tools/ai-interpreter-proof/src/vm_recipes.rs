// SPDX-License-Identifier: GPL-3.0-or-later
// Recipe definitions port core/ai.py, copyright IngeTrazo contributors.
//! Recipe bytecode executes only inside the genuine Rust interpreter.
use super::scene::console_scene::Scene;
use rustpython_vm::{PyRef, VirtualMachine, builtins::PyDictRef};
// Internal helper math only. This is not full stdlib `math` admission.
#[rustpython_vm::pymodule]
pub(super) mod _recipe_math {
    use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine};
    #[pyattr]
    const pi: f64 = std::f64::consts::PI;
    #[pyfunction]
    fn radians(v: PyObjectRef, vm: &VirtualMachine) -> PyResult<f64> {
        vm.host_budget_tick()?;
        let v = v.try_float(vm)?.to_f64();
        Ok(v * (std::f64::consts::PI / 180.0))
    }
    #[pyfunction]
    fn cos(v: PyObjectRef, vm: &VirtualMachine) -> PyResult<f64> {
        vm.host_budget_tick()?;
        let v = v.try_float(vm)?.to_f64();
        if v.is_infinite() {
            return Err(vm.new_value_error("math domain error"));
        }
        Ok(v.cos())
    }
    #[pyfunction]
    fn sin(v: PyObjectRef, vm: &VirtualMachine) -> PyResult<f64> {
        vm.host_budget_tick()?;
        let v = v.try_float(vm)?.to_f64();
        if v.is_infinite() {
            return Err(vm.new_value_error("math domain error"));
        }
        Ok(v.sin())
    }
    #[pyfunction]
    fn tan(v: PyObjectRef, vm: &VirtualMachine) -> PyResult<f64> {
        vm.host_budget_tick()?;
        let v = v.try_float(vm)?.to_f64();
        if v.is_infinite() {
            return Err(vm.new_value_error("math domain error"));
        }
        Ok(v.tan())
    }
    #[pyfunction]
    fn atan(v: PyObjectRef, vm: &VirtualMachine) -> PyResult<f64> {
        vm.host_budget_tick()?;
        Ok(v.try_float(vm)?.to_f64().atan())
    }
    #[pyfunction]
    fn hypot(a: PyObjectRef, b: PyObjectRef, vm: &VirtualMachine) -> PyResult<f64> {
        vm.host_budget_tick()?;
        Ok(a.try_float(vm)?.to_f64().hypot(b.try_float(vm)?.to_f64()))
    }
}
const SETUP: &str = r#"
def _recipes(scene):
    import _recipe_math as math
    from editable_mesh import Mesh
    from console_scene import Group, orient_owned
    from native_groups import QVector3D
    def _soften(mesh, deg=42.0):
        cos_t=math.cos(math.radians(deg))
        for e in mesh.edges:
            if len(e.faces)==2 and QVector3D.dotProduct(e.faces[0].normal(),e.faces[1].normal())>cos_t:
                e.soft=True
    def _finish(mesh,color,name):
        if color is not None:
            for f in mesh.faces:
                f.attrs['color']=tuple(color)
        orient_owned(mesh)
        _soften(mesh)
        group=Group(mesh,name=name)
        scene.groups.append(group)
        return group
    def extrude(outline,z0,z1,name=None,color=None):
        lo=[QVector3D(x,y,z0) for x,y in outline]
        hi=[QVector3D(x,y,z1) for x,y in outline]
        mesh=Mesh()
        n=len(lo)
        for i in range(n):
            j=(i+1)%n
            mesh.add_face([lo[i],lo[j],hi[j],hi[i]])
        mesh.add_face(list(reversed(lo)))
        mesh.add_face(hi)
        return _finish(mesh,color,name)
    def _pt(p,z=None):
        if hasattr(p,'x'):
            return QVector3D(p)
        if len(p)==2:
            return QVector3D(p[0],p[1],0.0 if z is None else z)
        return QVector3D(p[0],p[1],p[2])
    def _prism_mesh(mesh,loop,vec,holes=()):
        v=_pt(vec)
        lo=[_pt(p) for p in loop]
        hole_lo=[[_pt(p) for p in h] for h in holes or ()]
        mesh.add_face(lo,hole_lo)
        mesh.add_face([p+v for p in reversed(lo)],[[p+v for p in reversed(h)] for h in hole_lo])
        for ring in (lo,*hole_lo):
            n=len(ring)
            for i in range(n):
                a,b=ring[i],ring[(i+1)%n]
                mesh.add_face([a,b,b+v,a+v])
    def prism(loop,vec,holes=None,name=None,color=None):
        mesh=Mesh()
        _prism_mesh(mesh,loop,vec,holes or ())
        return _finish(mesh,color,name)
    def revolve(profile, segments=32, scallop=None, closed=False,
                name=None, color=None):
        """Solid of revolution around Z from an (radius, z) profile,
        bottom to top. Open profiles get end caps; ``closed=True`` treats
        the profile as a closed cross-section ring (e.g. a basin wall).
        ``scallop=(depth, lobes)`` carves festones INTO the radius, scaled
        by r/r_max so rims wave and centers stay put."""
        amp, lobes = scallop or (0.0, 0)
        rmax = max(r for r, _z in profile) or 1.0
        rows = []
        for r, z in profile:
            ring = []
            for i in range(segments):
                th = 2.0 * math.pi * i / segments
                rr = r
                if lobes:
                    rr += (amp * 0.5 * (math.cos(lobes * th) - 1.0)
                           * (r / rmax))
                ring.append(QVector3D(rr * math.cos(th),
                                      rr * math.sin(th), z))
            rows.append(ring)
        mesh = Mesh()
        m = len(rows)
        for j in range(m if closed else m - 1):
            a, b = rows[j], rows[(j + 1) % m]
            for i in range(segments):
                i2 = (i + 1) % segments
                quad = [a[i], a[i2], b[i2], b[i]]
                if ((quad[0] - quad[3]).length() < 1e-6
                        and (quad[1] - quad[2]).length() < 1e-6):
                    continue
                mesh.add_face(quad)
        if not closed:
            if profile[0][0] > 1e-6:
                mesh.add_face(list(reversed(rows[0])))
            if profile[-1][0] > 1e-6:
                mesh.add_face(rows[-1])
        return _finish(mesh, color, name)
    def _wall_mesh(mesh, a, b, height, thickness, openings=(), z0=0.0,
                   peak=None):
        """One wall into ``mesh``: base line a→b (x, y), the solid grows to
        the LEFT of a→b by ``thickness`` (walk the footprint counter-
        clockwise and the inside is the left). ``openings`` are
        ``(offset, sill, width, height)`` along the wall from ``a``; a sill
        at 0 is a door (a notch in the outline), above 0 a window (a hole).
        ``peak=(offset, rise)`` raises the top to a gable point."""
        ax, ay = a[0], a[1]
        bx, by = b[0], b[1]
        length = math.hypot(bx - ax, by - ay)
        if length < 1e-9:
            return
        ux, uy = (bx - ax) / length, (by - ay) / length
        nx, ny = -uy, ux                      # left of a→b
        top = z0 + height

        def P(s, z):
            return QVector3D(ax + ux * s, ay + uy * s, z)
        doors = sorted((o for o in openings if o[1] <= 1e-6), key=lambda o: o[0])
        windows = [o for o in openings if o[1] > 1e-6]
        outline = [P(0.0, z0)]
        for off, _sill, w, h in doors:
            s0, s1 = max(off, 0.0), min(off + w, length)
            if s1 - s0 < 1e-6:
                continue
            outline += [P(s0, z0), P(s0, z0 + h), P(s1, z0 + h), P(s1, z0)]
        outline.append(P(length, z0))
        outline.append(P(length, top))
        if peak is not None:
            outline.append(P(peak[0], top + peak[1]))
        outline.append(P(0.0, top))
        # Drop the doubled corner when a door starts at 0.
        clean = []
        for q in outline:
            if not clean or (q - clean[-1]).length() > 1e-9:
                clean.append(q)
        if (clean[0] - clean[-1]).length() < 1e-9:
            clean.pop()
        holes = []
        for off, sill, w, h in windows:
            s0, s1 = max(off, 0.0), min(off + w, length)
            zt = min(z0 + sill + h, top - 0.05)
            if s1 - s0 < 1e-6 or zt - (z0 + sill) < 1e-6:
                continue
            holes.append([P(s0, z0 + sill), P(s0, zt), P(s1, zt), P(s1, z0 + sill)])
        _prism_mesh(mesh, clean, (nx * thickness, ny * thickness, 0.0), holes)
    def wall(a, b, height=3.0, thickness=0.2, openings=(), z0=0.0,
             peak=None, name=None, color=None):
        """A wall with thickness and openings — see ``house`` for the
        conventions. ``openings=[(offset, sill, width, height), ...]``."""
        mesh = Mesh()
        _wall_mesh(mesh, a, b, height, thickness, openings, z0, peak)
        return _finish(mesh, color, name)
    def house(width=6.0, depth=4.0, wall_height=3.0, thickness=0.2,
              roof="gable", ridge_height=None, overhang=0.4, ridge="x",
              doors=(), windows=(), origin=(0.0, 0.0), name="Casa",
              wall_color=(0.90, 0.87, 0.80, 1.0),
              roof_color=(0.55, 0.27, 0.20, 1.0),
              door_color=(0.45, 0.28, 0.15, 1.0),
              glass_color=(0.60, 0.80, 0.95, 1.0)):
        """A whole house in one call: four walls with thickness, doors and
        windows cut through them (with a door leaf and a glass pane), and
        a roof — ``"gable"`` (two slopes, ridge along ``"x"`` or ``"y"``),
        ``"hip"`` (four slopes) or ``"flat"``. Sides are ``"S"`` (y = y0,
        the front), ``"E"``, ``"N"``, ``"W"``; an opening's ``offset`` runs
        along the wall counter-clockwise from its first corner (S from
        x0, E from y0, N from x1 back, W from y1 back). ``doors=[(side,
        offset, width, height)]``, ``windows=[(side, offset, sill, width,
        height)]``. Returns the groups: walls, floor, carpentry, roof."""
        x0, y0 = float(origin[0]), float(origin[1])
        x1, y1 = x0 + float(width), y0 + float(depth)
        H, t = float(wall_height), float(thickness)
        if ridge_height is None:
            half = (depth if ridge == "x" else width) / 2.0
            ridge_height = half * math.tan(math.radians(30.0))
        rh = float(ridge_height)
        lines = {"S": ((x0, y0), (x1, y0)), "E": ((x1, y0), (x1, y1)),
                 "N": ((x1, y1), (x0, y1)), "W": ((x0, y1), (x0, y0))}
        # E and W sit between S and N, so the corners are not doubled.
        lines["E"] = ((x1, y0 + t), (x1, y1 - t))
        lines["W"] = ((x0, y1 - t), (x0, y0 + t))
        shift = {"S": 0.0, "N": 0.0, "E": -t, "W": -t}
        ops: dict = {k: [] for k in lines}
        leaves: list = []          # (side, offset, sill, w, h, kind)
        for side, off, w, h in doors:
            ops[side.upper()].append((off + shift[side.upper()], 0.0, w, h))
            leaves.append((side.upper(), off + shift[side.upper()], 0.0, w, h, "door"))
        for side, off, sill, w, h in windows:
            ops[side.upper()].append((off + shift[side.upper()], sill, w, h))
            leaves.append((side.upper(), off + shift[side.upper()], sill, w, h, "glass"))
        gable_sides = ()
        if roof == "gable":
            gable_sides = ("E", "W") if ridge == "x" else ("S", "N")
        walls_mesh = Mesh()
        for side, (a, b) in lines.items():
            peak = None
            if side in gable_sides:
                seg = math.hypot(b[0] - a[0], b[1] - a[1])
                peak = (seg / 2.0, rh)
            _wall_mesh(walls_mesh, a, b, H, t, ops[side], 0.0, peak)
        out = [_finish(walls_mesh, wall_color, f"{name} · Paredes")]
        # A floor slab just inside the walls (a hair short of them, so no
        # two faces share a plane), or the house reads hollow from below.
        floor = Mesh()
        e = t + 0.01
        _prism_mesh(floor, [QVector3D(x0 + e, y0 + e, 0.0), QVector3D(x1 - e, y0 + e, 0.0),
                            QVector3D(x1 - e, y1 - e, 0.0), QVector3D(x0 + e, y1 - e, 0.0)],
                    (0.0, 0.0, 0.1))
        out.append(_finish(floor, (0.75, 0.72, 0.66, 1.0), f"{name} · Piso"))
        # Carpentry: a door leaf and a glass pane in every opening.
        carp = Mesh()
        for side, off, sill, w, h, kind in leaves:
            a, b = lines[side]
            length = math.hypot(b[0] - a[0], b[1] - a[1])
            ux, uy = (b[0] - a[0]) / length, (b[1] - a[1]) / length
            nx, ny = -uy, ux
            d = t * 0.5
            s0, s1 = off, off + w
            base = [QVector3D(a[0] + ux * s0 + nx * d, a[1] + uy * s0 + ny * d, sill),
                    QVector3D(a[0] + ux * s1 + nx * d, a[1] + uy * s1 + ny * d, sill),
                    QVector3D(a[0] + ux * s1 + nx * d, a[1] + uy * s1 + ny * d, sill + h),
                    QVector3D(a[0] + ux * s0 + nx * d, a[1] + uy * s0 + ny * d, sill + h)]
            if kind == "door":
                _prism_mesh(carp, base, (nx * 0.04, ny * 0.04, 0.0))
                for f in carp.faces:
                    f.attrs.setdefault("color", tuple(door_color))
            else:
                f = carp.add_face(base)
                f.attrs["color"] = tuple(glass_color)
                f.attrs["opacity"] = 0.4
        if carp.faces:
            out.append(_finish(carp, None, f"{name} · Carpintería"))
        # Roof.
        roof_mesh = Mesh()
        ov = float(overhang)
        if roof == "flat":
            _prism_mesh(roof_mesh, [QVector3D(x0 - ov, y0 - ov, H), QVector3D(x1 + ov, y0 - ov, H),
                                    QVector3D(x1 + ov, y1 + ov, H), QVector3D(x0 - ov, y1 + ov, H)],
                        (0.0, 0.0, 0.2))
        elif roof == "hip":
            L, W = x1 - x0, y1 - y0
            cx, cy = (x0 + x1) / 2.0, (y0 + y1) / 2.0
            s = rh / (min(L, W) / 2.0)
            ze = H - s * ov
            b0, b1, b2, b3 = (QVector3D(x0 - ov, y0 - ov, ze), QVector3D(x1 + ov, y0 - ov, ze),
                              QVector3D(x1 + ov, y1 + ov, ze), QVector3D(x0 - ov, y1 + ov, ze))
            if L >= W:
                r0, r1 = QVector3D(cx - (L - W) / 2.0, cy, H + rh), QVector3D(cx + (L - W) / 2.0, cy, H + rh)
                faces = [[b0, b1, r1, r0], [b2, b3, r0, r1], [b1, b2, r1], [b3, b0, r0]]
            else:
                r0, r1 = QVector3D(cx, cy - (W - L) / 2.0, H + rh), QVector3D(cx, cy + (W - L) / 2.0, H + rh)
                faces = [[b1, b2, r1, r0], [b3, b0, r0, r1], [b0, b1, r0], [b2, b3, r1]]
            for f in faces:
                roof_mesh.add_face(f)
            roof_mesh.add_face([b3, b2, b1, b0])
        else:                                   # gable: a bent slab
            th = 0.15
            if ridge == "x":
                s = rh / ((y1 - y0) / 2.0)
                ze, yc = H - s * ov, (y0 + y1) / 2.0
                dz = th / math.cos(math.atan(s))
                sec = [QVector3D(x0 - ov, y0 - ov, ze), QVector3D(x0 - ov, yc, H + rh),
                       QVector3D(x0 - ov, y1 + ov, ze), QVector3D(x0 - ov, y1 + ov, ze - dz),
                       QVector3D(x0 - ov, yc, H + rh - dz), QVector3D(x0 - ov, y0 - ov, ze - dz)]
                _prism_mesh(roof_mesh, sec, (x1 - x0 + 2 * ov, 0.0, 0.0))
            else:
                s = rh / ((x1 - x0) / 2.0)
                ze, xc = H - s * ov, (x0 + x1) / 2.0
                dz = th / math.cos(math.atan(s))
                sec = [QVector3D(x0 - ov, y0 - ov, ze), QVector3D(xc, y0 - ov, H + rh),
                       QVector3D(x1 + ov, y0 - ov, ze), QVector3D(x1 + ov, y0 - ov, ze - dz),
                       QVector3D(xc, y0 - ov, H + rh - dz), QVector3D(x0 - ov, y0 - ov, ze - dz)]
                _prism_mesh(roof_mesh, sec, (0.0, y1 - y0 + 2 * ov, 0.0))
        out.append(_finish(roof_mesh, roof_color, f"{name} · Techo"))
        return out

    return {'extrude':extrude,'prism':prism,'revolve':revolve,'wall':wall,'house':house}
"#;
pub(super) fn prepare(scene: PyRef<Scene>, vm: &VirtualMachine) -> Result<PyDictRef, String> {
    let scope = vm.new_scope_with_builtins();
    let code = vm
        .compile(SETUP, rustpython_vm::compiler::Mode::Exec, "<recipes>")
        .map_err(|_| "Recipe compilation refused")?;
    vm.run_code_obj(code, scope.clone())
        .map_err(|_| "Recipe setup refused")?;
    scope
        .globals
        .get_item("_recipes", vm)
        .map_err(|_| "Recipe factory missing")?
        .call((scene,), vm)
        .map_err(|e| {
            vm.print_exception(&e);
            "Recipe bindings refused"
        })?
        .downcast::<rustpython_vm::builtins::PyDict>()
        .map_err(|_| "Recipe mapping requires dict".into())
}
