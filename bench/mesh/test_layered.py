import numpy as np

import layered as L


def test_rotation_is_right_handed_about_every_axis():
    for axis, p, want in (("x", [0, 1, 0], [0, 0, 1]), ("y", [0, 0, 1], [1, 0, 0]), ("z", [1, 0, 0], [0, 1, 0])):
        assert np.allclose(L.rotation(axis, 90) @ np.array(p, float), want), axis


def test_positive_rotation_about_y_lowers_a_point_on_plus_x():
    v = np.array([[30.0, 0, 30.0]])
    out = L.apply_ops(v, np.zeros((0, 3), int), [("select", [30, 0, 30], 5, 5), ("rotate", "y", 40, [20, 0, 30])])
    assert out[0, 2] < 30


if __name__ == "__main__":
    for name, fn in list(globals().items()):
        if name.startswith("test_"):
            fn()
            print("ok", name)
