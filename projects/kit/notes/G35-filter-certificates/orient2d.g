# orient2d's filter (geom_num::generated::orient2d::filter), as Gappa states it: the control
# problem of the spike, the smallest homogeneous predicate. Inputs: the four coordinate
# differences, whole numbers (|v| <= 2^41 - 2, exact in f64), and the two axis maxima m_x and m_y,
# each the largest |difference| of its axis and at least 1 here (a zero maximum makes the scale
# zero, which the filter answers exactly).

@rnd = float<ieee_64, ne>;

det_1 = rnd(ba_x * ca_y);
det_2 = rnd(ba_y * ca_x);
det = rnd(det_1 - det_2);

Det = ba_x * ca_y - ba_y * ca_x;

S = m_x * m_y;
scale = rnd(m_x * m_y);
C = 0x1.0000000000003p-51;
bound = rnd(C * scale);

{
  @FIX(ba_x, 0) /\ @FIX(ba_y, 0) /\ @FIX(ca_x, 0) /\ @FIX(ca_y, 0) /\
  @FIX(m_x, 0) /\ @FIX(m_y, 0) /\
  m_x in [1, 2199023255550] /\ m_y in [1, 2199023255550] /\
  ba_x // m_x in [-1, 1] /\ ca_x // m_x in [-1, 1] /\
  ba_y // m_y in [-1, 1] /\ ca_y // m_y in [-1, 1]
->
  det - Det // S in ? /\ bound -/ C * S in ?
}

# A product's scale is the same whichever order its factors come in.
m_x * m_y -> m_y * m_x;
m_y * m_x -> m_x * m_y;
