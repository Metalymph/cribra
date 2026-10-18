"""Tag the prebuilt ctypes library as platform-specific, without a CPython ABI."""

import os

from setuptools import Distribution, setup
from setuptools.command.bdist_wheel import bdist_wheel


class NativeDistribution(Distribution):
    def has_ext_modules(self):
        return True


class NativeWheel(bdist_wheel):
    def get_tag(self):
        return "py3", "none", os.environ["CRIBRA_WHEEL_PLATFORM"]


setup(distclass=NativeDistribution, cmdclass={"bdist_wheel": NativeWheel})
