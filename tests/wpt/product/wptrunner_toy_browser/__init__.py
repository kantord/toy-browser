"""This browser, as something wptrunner knows how to drive.

Almost nothing of our own. wptrunner already knows how to drive anything that
speaks WebDriver, and this browser does — so what is left is the command line
that starts it, and saying which executor handles which kind of test.
"""

from wptrunner.browsers.base import WebDriverBrowser, get_timeout_multiplier, require_arg
from wptrunner.executors import executor_kwargs as base_executor_kwargs
from wptrunner.executors.base import PytestExecutor
from wptrunner.executors.executorwebdriver import (
    WebDriverCrashtestExecutor,
    WebDriverRefTestExecutor,
    WebDriverTestharnessExecutor,
)
from wptrunner.products import Product

__all__ = [
    "PytestExecutor",
    "ToyBrowser",
    "WebDriverCrashtestExecutor",
    "WebDriverRefTestExecutor",
    "WebDriverTestharnessExecutor",
    "get_timeout_multiplier",
    "product",
]


class ToyBrowser(WebDriverBrowser):
    """Started the way the command line starts it."""

    def make_command(self):
        return [
            self.webdriver_binary,
            "webdriver",
            "--port",
            str(self.port),
        ] + self.webdriver_args


def check_args(**kwargs):
    require_arg(kwargs, "webdriver_binary")


def browser_kwargs(logger, test_type, run_info_data, config, **kwargs):
    return {
        "binary": kwargs["binary"],
        "webdriver_binary": kwargs["webdriver_binary"],
        "webdriver_args": kwargs.get("webdriver_args"),
    }


def executor_kwargs(logger, test_type, test_environment, run_info_data, **kwargs):
    built = base_executor_kwargs(test_type, test_environment, run_info_data, **kwargs)
    built["capabilities"] = {}
    return built


def env_options():
    return {}


def env_extras(**kwargs):
    return []


__wptrunner__ = {
    "product": "toy_browser",
    "check_args": "check_args",
    "browser": "ToyBrowser",
    "browser_kwargs": "browser_kwargs",
    "executor_kwargs": "executor_kwargs",
    "env_options": "env_options",
    "env_extras": "env_extras",
    "timeout_multiplier": "get_timeout_multiplier",
    "executor": {
        "testharness": "WebDriverTestharnessExecutor",
        "reftest": "WebDriverRefTestExecutor",
        "wdspec": "PytestExecutor",
        "crashtest": "WebDriverCrashtestExecutor",
    },
}


def product() -> Product:
    """What the entry point hands back: this module, read as a product."""
    import sys

    return Product._from_dunder_wptrunner(sys.modules[__name__])
