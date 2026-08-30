class Ghdash < Formula
  desc "TUI GitHub dashboard for monitoring repos, PRs, and review inbox"
  homepage "https://github.com/zombocoder/ghdash"
  version "0.3.3"
  license "Apache-2.0"

  on_macos do
    on_arm do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-aarch64-apple-darwin.tar.gz"
      sha256 "acc06a979b3aaa3db6d5ef60817115a59349a4788e63e658c232cd17d282ddc2"
    end
    on_intel do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-x86_64-apple-darwin.tar.gz"
      sha256 "bdbb1f067931484dacda43cdd6250a56a836b5e75da18be1717c66b3d8138b40"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "be0b0b695e2ef2941ff37aafa44dfc595e616fd6a659cbeba2e2714d3988c8c8"
    end
    on_intel do
      url "https://github.com/zombocoder/ghdash/releases/download/v#{version}/ghdash-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "20d5ab9937eb7d03d898a25b7f8da5c9f93ee96a961ab1544e9d32f5cf7f9f84"
    end
  end

  def install
    bin.install "ghdash"
  end

  test do
    assert_match "ghdash", shell_output("#{bin}/ghdash --version")
  end
end
