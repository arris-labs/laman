"""Hypothesis profiles: `default` keeps py-test.sh under a minute, `ci` raises
the example count (`HYPOTHESIS_PROFILE=ci`). No deadline: a solve's time is
not what the property checks."""

import os

from hypothesis import HealthCheck, settings

settings.register_profile(
    "default", max_examples=500, deadline=None, suppress_health_check=[HealthCheck.too_slow]
)
settings.register_profile(
    "ci", max_examples=5000, deadline=None, suppress_health_check=[HealthCheck.too_slow]
)
settings.load_profile(os.environ.get("HYPOTHESIS_PROFILE", "default"))
