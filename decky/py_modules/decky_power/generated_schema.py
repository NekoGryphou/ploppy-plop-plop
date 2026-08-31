"""Generated from proto/decky_power.proto. Do not edit by hand."""

SCHEMA_SHA256 = "398d4cbde7f18430b397ad50b38a54d3e6a7740abc04ba34987870fb42134832"
MESSAGES: dict[str, dict[str, tuple[int, str]]] = {
    "StatusRequest": {
        "protocol_version": (1, "uint32"),
        "client_version": (2, "string"),
    },
    "StatusResponse": {
        "hostname": (1, "string"),
        "host_version": (2, "string"),
        "protocol_version": (3, "uint32"),
        "paired": (4, "bool"),
        "host_id": (5, "string"),
    },
    "PairRequest": {
        "protocol_version": (1, "uint32"),
        "client_spake2_message": (2, "bytes"),
        "session_id": (3, "bytes"),
        "client_confirmation": (4, "bytes"),
        "client_version": (5, "string"),
    },
    "PairResponse": {
        "host_spake2_message": (1, "bytes"),
        "encryption_nonce": (2, "bytes"),
        "encrypted_credential": (3, "bytes"),
        "hostname": (4, "string"),
        "host_version": (5, "string"),
        "protocol_version": (6, "uint32"),
        "host_id": (7, "string"),
        "session_id": (8, "bytes"),
    },
    "ShutdownRequest": {
        "protocol_version": (1, "uint32"),
        "client_version": (2, "string"),
    },
    "ShutdownResponse": {
        "accepted": (1, "bool"),
    },
    "ErrorResponse": {
        "code": (1, "enum"),
        "message": (2, "string"),
    },
}
