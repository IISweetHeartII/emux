# Homebrew formula for emux
# To use: create a tap repo (IISweetHeartII/homebrew-tap) and place this file
# in Formula/emux.rb, then users can install with:
#   brew tap IISweetHeartII/tap
#   brew install emux

class Emux < Formula
  desc "A modern terminal multiplexer — zero config, session persistence, cross-platform"
  homepage "https://github.com/IISweetHeartII/emux"
  license "MIT"

  # Update these for each release
  version "0.1.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/IISweetHeartII/emux/releases/download/v#{version}/emux-v#{version}-aarch64-apple-darwin.tar.gz"
      # sha256 "UPDATE_WITH_ACTUAL_SHA256"
    else
      url "https://github.com/IISweetHeartII/emux/releases/download/v#{version}/emux-v#{version}-x86_64-apple-darwin.tar.gz"
      # sha256 "UPDATE_WITH_ACTUAL_SHA256"
    end
  end

  on_linux do
    url "https://github.com/IISweetHeartII/emux/releases/download/v#{version}/emux-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    # sha256 "UPDATE_WITH_ACTUAL_SHA256"
  end

  def install
    bin.install "emux"
  end

  test do
    assert_match "emux", shell_output("#{bin}/emux --version")
  end
end
