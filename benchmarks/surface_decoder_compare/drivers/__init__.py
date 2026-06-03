from .ilpqec_driver import IlpqecDriver
from .ldpc_driver import LdpcDriver
from .pymatching_driver import PymatchingDriver
from .rust_bridge import RustBridgeDriver


def build_driver_registry() -> dict[str, object]:
    return {
        "pymatching": PymatchingDriver(),
        "ilpqec": IlpqecDriver(),
        "ldpc": LdpcDriver(),
        "rmatching": RustBridgeDriver("rmatching"),
        "rbposd": RustBridgeDriver("rbposd"),
        "rilpqec": RustBridgeDriver("rilpqec"),
    }
