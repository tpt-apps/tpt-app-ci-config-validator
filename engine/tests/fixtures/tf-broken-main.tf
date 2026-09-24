# Deliberately broken: unbalanced braces in the first resource block, a
# resource block with only one label, duplicated resource address, and a
# hardcoded credential.
terraform {
  required_version = ">= 1.5.0"
}

provider "aws" {
  region = "ap-southeast-2"
}

resource "aws_s3_bucket" "artifacts" {
  bucket = "tpt-ci-artifacts"
  tags = {
    Team = "platform"

resource "aws_s3_bucket" "artifacts" {
  bucket = "tpt-ci-artifacts-dup"
}

resource "aws_instance" {
  ami = "ami-12345678"
}

resource "aws_db_instance" "db" {
  engine  = "postgres"
  password = "Sup3rS3cretP@ss!"
}
