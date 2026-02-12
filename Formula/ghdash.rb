class Ghdash < Formula
  desc "TUI GitHub dashboard for monitoring repos, PRs, and review inbox"
  homepage "https://github.com/zombocoder/ghdash"
  version "0.3.0"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-aarch64-apple-darwin.tar.gz"
      # sha256 "PLACEHOLDER" # Updated automatically by release workflow
    end
    on_intel do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-x86_64-apple-darwin.tar.gz"
      # sha256 "PLACEHOLDER" # Updated automatically by release workflow
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-aarch64-unknown-linux-gnu.tar.gz"
      # sha256 "PLACEHOLDER" # Updated automatically by release workflow
    end
    on_intel do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-x86_64-unknown-linux-gnu.tar.gz"
      # sha256 "PLACEHOLDER" # Updated automatically by release workflow
    end
  end

  def install
    bin.install "ghdash"
  end

  test do
    assert_match "ghdash", shell_output("#{bin}/ghdash --version")
  end
end
