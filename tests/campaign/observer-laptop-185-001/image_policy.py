"""Pure exploratory sampled-image classification."""

ALLOWED = frozenset({
    "pinned_enclosing_guard", "campaign_member", "unclassified_background",
})


def classify(name, image_sha256, *, enclosing, member, compiler_names, veto_sha256):
    if enclosing:
        return "pinned_enclosing_guard"
    if member:
        return "campaign_member"
    if name.casefold() in compiler_names:
        return "compiler_present"
    if image_sha256 in veto_sha256:
        return "pinned_workload_outside_job"
    return "unclassified_background"
