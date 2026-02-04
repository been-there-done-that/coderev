class Coderev < Formula
  desc "A powerful AI-driven grep tool"
  homepage "https://github.com/been-there-done-that/coderev"
  url "https://github.com/been-there-done-that/coderev/releases/download/v0.1.0/coderev-x86_64-apple-darwin.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  version "0.1.0"

  def install
    bin.install "coderev"
  end

  test do
    system "#{bin}/coderev", "--version"
  end
end
