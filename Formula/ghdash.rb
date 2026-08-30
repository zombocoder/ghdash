class Ghdash < Formula
  desc "TUI GitHub dashboard for monitoring repos, PRs, and review inbox"
  homepage "https://github.com/zombocoder/ghdash"
  version "0.3.3"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-aarch64-apple-darwin.tar.gz"
      sha256 "cdb86f77d8cbb3e7362e2d2668e7b6aa954848c3557f93f76f6837a991db8ddd"
    end
    on_intel do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-x86_64-apple-darwin.tar.gz"
      sha256 "aa2048fd3f42d63c3ea1a950f890c4e0d00dfd6fa73f241adeccdee6f00ca426"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "5521d5f11ae54b6b08023bad6f5398eaf8944b0e3f7b2a13f5cb1c128ab1d4e5"
    end
    on_intel do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "92da4561181bee0956bab5672c092094f195294596fc3e922d4e204871d2d76c"
    end
  end

  def install
    bin.install "ghdash"
  end

  test do
    assert_match "ghdash", shell_output("#{bin}/ghdash --version")
  end
end
