//! Two f64 columns, x and y, in a ring of fixed capacity. x is ascending in
//! logical order (oldest first). Static data is a ring that is exactly full.

pub struct Series {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    cap: usize,
    len: usize,
    /// The physical index the next push writes.
    head: usize,
}

impl Series {
    pub fn new(cap: usize) -> Self {
        Self { x: Vec::new(), y: Vec::new(), cap: cap.max(1), len: 0, head: 0 }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Room for `n` points written straight into `x` and `y` by the caller
    /// (zero-copy from JS through a Float64Array view), then `commit(n)`.
    pub fn reserve(&mut self, n: usize) {
        self.cap = n.max(1);
        self.x = vec![0.0; self.cap];
        self.y = vec![0.0; self.cap];
        self.len = 0;
        self.head = 0;
    }

    pub fn commit(&mut self, n: usize) {
        self.len = n.min(self.cap);
        self.head = self.len % self.cap;
    }

    pub fn replace(&mut self, x: &[f64], y: &[f64]) {
        let n = x.len().min(y.len());
        self.reserve(n.max(self.cap));
        self.x[..n].copy_from_slice(&x[..n]);
        self.y[..n].copy_from_slice(&y[..n]);
        self.commit(n);
    }

    #[inline]
    pub fn push(&mut self, x: f64, y: f64) {
        if self.x.len() < self.cap {
            self.x.resize(self.cap, 0.0);
            self.y.resize(self.cap, 0.0);
        }
        self.x[self.head] = x;
        self.y[self.head] = y;
        self.head = (self.head + 1) % self.cap;
        self.len = (self.len + 1).min(self.cap);
    }

    pub fn push_batch(&mut self, x: &[f64], y: &[f64]) {
        for (a, b) in x.iter().zip(y) {
            self.push(*a, *b);
        }
    }

    #[inline]
    fn start(&self) -> usize {
        if self.len < self.cap {
            0
        } else {
            self.head
        }
    }

    #[inline]
    pub fn x_at(&self, i: usize) -> f64 {
        self.x[(self.start() + i) % self.cap]
    }

    #[inline]
    pub fn y_at(&self, i: usize) -> f64 {
        self.y[(self.start() + i) % self.cap]
    }

    /// The first logical index whose x is at least `v`.
    pub fn lower_bound(&self, v: f64) -> usize {
        let (mut lo, mut hi) = (0, self.len);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.x_at(mid) < v {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Calls `f(x, y)` for logical indices `i0..i1`, as at most two slices.
    #[inline]
    pub fn each(&self, i0: usize, i1: usize, mut f: impl FnMut(f64, f64)) {
        if i0 >= i1 {
            return;
        }
        let s = self.start();
        let (a, b) = (s + i0, s + i1);
        let mut run = |p0: usize, p1: usize| {
            for (x, y) in self.x[p0..p1].iter().zip(&self.y[p0..p1]) {
                f(*x, *y);
            }
        };
        if b <= self.cap {
            run(a, b);
        } else if a >= self.cap {
            run(a - self.cap, b - self.cap);
        } else {
            run(a, self.cap);
            run(0, b - self.cap);
        }
    }
}
