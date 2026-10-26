# orient3d's filter (geom_num::generated::orient3d::filter), as Gappa states it.
# Inputs: the nine coordinate differences, whole numbers (|v| <= 2^41 - 2, exact in f64), and the
# three axis maxima m_x, m_y, m_z, each the largest |difference| of its axis and at least 1 here
# (a zero maximum makes the scale zero, which the filter answers exactly).

@rnd = float<ieee_64, ne>;

n_x_1 = rnd(ca_y * da_z);
n_x_2 = rnd(ca_z * da_y);
n_x = rnd(n_x_1 - n_x_2);
n_y_1 = rnd(ca_z * da_x);
n_y_2 = rnd(ca_x * da_z);
n_y = rnd(n_y_1 - n_y_2);
n_z_1 = rnd(ca_x * da_y);
n_z_2 = rnd(ca_y * da_x);
n_z = rnd(n_z_1 - n_z_2);
det_1 = rnd(ba_x * n_x);
det_2 = rnd(ba_y * n_y);
det_3 = rnd(det_1 + det_2);
det_4 = rnd(ba_z * n_z);
det = rnd(det_3 + det_4);

# The exact value, in the same shape.
Nx = ca_y * da_z - ca_z * da_y;
Ny = ca_z * da_x - ca_x * da_z;
Nz = ca_x * da_y - ca_y * da_x;
Det = ba_x * Nx + ba_y * Ny + ba_z * Nz;

# The scale and the bound as the filter computes them.
S = m_x * m_y * m_z;
scale = rnd(rnd(m_x * m_y) * m_z);
C = 0x1.c000000000007p-49;
bound = rnd(C * scale);

{
  @FIX(ba_x, 0) /\ @FIX(ba_y, 0) /\ @FIX(ba_z, 0) /\
  @FIX(ca_x, 0) /\ @FIX(ca_y, 0) /\ @FIX(ca_z, 0) /\
  @FIX(da_x, 0) /\ @FIX(da_y, 0) /\ @FIX(da_z, 0) /\
  @FIX(m_x, 0) /\ @FIX(m_y, 0) /\ @FIX(m_z, 0) /\
  m_x in [1, 2199023255550] /\ m_y in [1, 2199023255550] /\ m_z in [1, 2199023255550] /\
  ba_x // m_x in [-1, 1] /\ ca_x // m_x in [-1, 1] /\ da_x // m_x in [-1, 1] /\
  ba_y // m_y in [-1, 1] /\ ca_y // m_y in [-1, 1] /\ da_y // m_y in [-1, 1] /\
  ba_z // m_z in [-1, 1] /\ ca_z // m_z in [-1, 1] /\ da_z // m_z in [-1, 1]
->
  det - Det // S in ? /\ bound -/ C * S in ?
}

# The error of a product of an exact difference and a computed value is that difference times the
# computed value's error.
ba_x * n_x - ba_x * Nx -> ba_x * (n_x - Nx);
ba_y * n_y - ba_y * Ny -> ba_y * (n_y - Ny);
ba_z * n_z - ba_z * Nz -> ba_z * (n_z - Nz);

# Products of maxima are the same scale whatever order a step forms them in, in either direction.
m_y * m_z -> m_z * m_y;
m_z * m_y -> m_y * m_z;
m_x * m_z -> m_z * m_x;
m_z * m_x -> m_x * m_z;
m_x * m_y -> m_y * m_x;
m_y * m_x -> m_x * m_y;
S -> m_x * (m_y * m_z);
S -> m_y * (m_x * m_z);
S -> m_z * (m_x * m_y);
m_x * (m_y * m_z) -> S;
m_y * (m_x * m_z) -> S;
m_z * (m_x * m_y) -> S;
