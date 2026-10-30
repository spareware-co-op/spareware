// The Clothoids oracle's harness (piece G63; chapter 28.10's out-of-process oracles): reads
// lines of seven numbers, x0 y0 theta0 k dk L s, from standard input, and writes the point of the
// clothoid G2lib::ClothoidCurve(x0, y0, theta0, k, dk, L) at arc length s as two numbers of 17
// significant digits, which read back to the same doubles, so every value crosses the process
// boundary bit for bit. It only calls the library: built against Bertolazzi and Frego's Clothoids
// (BSD-2-Clause) at the pinned commit, it runs as its own process and is never linked into the
// kernel. The kernel's side is geom-curve's tests/clothoids_oracle.rs.
#include <cstdio>

#include "Clothoids.hh"

int main() {
  double x0, y0, theta0, k, dk, length, s;
  while (std::scanf("%la %la %la %la %la %la %la", &x0, &y0, &theta0, &k, &dk, &length, &s) == 7) {
    G2lib::ClothoidCurve curve(x0, y0, theta0, k, dk, length, "oracle");
    double x = 0.0, y = 0.0;
    curve.eval(s, x, y);
    std::printf("%.17g %.17g\n", x, y);
  }
  return 0;
}
