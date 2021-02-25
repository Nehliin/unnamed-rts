use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glam::*;

#[derive(Debug, PartialEq, Clone, Copy)]
struct NonNan(f32);

impl NonNan {
    fn new(val: f32) -> Option<NonNan> {
        if val.is_nan() {
            None
        } else {
            Some(NonNan(val))
        }
    }
}

impl Eq for NonNan {}

impl PartialOrd for NonNan {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for NonNan {
    fn cmp(&self, other: &NonNan) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn old_intersects(origin: Vec3, dirfrac: Vec3, aabb_min: Vec3, aabb_max: Vec3) -> bool {
    use std::cmp::{max, min};
    let t1 = NonNan::new((aabb_min.x - origin.x) * dirfrac.x).unwrap();
    let t2 = NonNan::new((aabb_max.x - origin.x) * dirfrac.x).unwrap();
    let t3 = NonNan::new((aabb_min.y - origin.y) * dirfrac.y).unwrap();
    let t4 = NonNan::new((aabb_max.y - origin.y) * dirfrac.y).unwrap();
    let t5 = NonNan::new((aabb_min.z - origin.z) * dirfrac.z).unwrap();
    let t6 = NonNan::new((aabb_max.z - origin.z) * dirfrac.z).unwrap();

    let tmin = max(max(min(t1, t2), min(t3, t4)), min(t5, t6));
    let tmax = min(min(max(t1, t2), max(t3, t4)), max(t5, t6));

    if tmax < NonNan::new(0.0).unwrap() || tmax < tmin {
        return false;
    }
    true
}

fn intesercts(origin: Vec3A, dirfrac: Vec3A, aabb_min: Vec3A, aabb_max: Vec3A) -> bool {
    let t1 = (aabb_min - origin) * dirfrac;
    let t2 = (aabb_max - origin) * dirfrac;

    let tmin = t1.min(t2);
    let tmin = tmin.max_element();

    let tmax = t1.max(t2);
    let tmax = tmax.min_element();

    !(tmax < 0.0 || tmax < tmin)
}

fn old_intersection(c: &mut Criterion) {
    c.bench_function("old intersection", |b| {
        b.iter(|| {
            old_intersects(
                black_box(Vec3::new(0.0, 2.0, 3.5)),
                black_box(Vec3::new(2.1791692, 138.40993, -1.125587)),
                black_box(Vec3::new(-2.8, 0.0, -0.4)),
                black_box(Vec3::new(-1.2, 3.2, 0.4)),
            )
        })
    });
}

fn new_intersection(c: &mut Criterion) {
    c.bench_function("new intersection", |b| {
        b.iter(|| {
            intesercts(
                black_box(Vec3A::new(0.0, 2.0, 3.5)),
                black_box(Vec3A::new(2.1791692, 138.40993, -1.125587)),
                black_box(Vec3A::new(-2.8, 0.0, -0.4)),
                black_box(Vec3A::new(-1.2, 3.2, 0.4)),
            )
        })
    });
}
criterion_group!(benches, old_intersection, new_intersection);
criterion_main!(benches);
