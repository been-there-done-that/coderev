class Coderev < Formula
  desc "A powerful AI-driven grep tool"
  homepage "https://github.com/been-there-done-that/coderev"
  url "https://github.com/been-there-done-that/coderev/releases/download/v0.0.1/coderev-x86_64-apple-darwin.tar.gz"
  sha256 "a9d3f93aec52f410c74721354ec991a7f5d6b5fb3512d19a6e0ae58a619385c4"
  version "0.0.1"

  def install
    bin.install "coderev"
  end

  test do
    system "#{bin}/coderev", "--version"
  end
end
