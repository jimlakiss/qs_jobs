class User < ApplicationRecord
  # Include default devise modules. Others available are:
  # :confirmable, :lockable, :timeoutable, :trackable and :omniauthable
  devise  :database_authenticatable, 
          :recoverable, 
          :rememberable, 
          :validatable

  enum :role, { admin: 0, client: 1 }, default: :admin

  belongs_to :contributor, optional: true
  has_many :client_submissions, foreign_key: :submitted_by_id, dependent: :restrict_with_error
  has_many :received_project_issues, class_name: "ProjectIssue", foreign_key: :recipient_user_id, dependent: :restrict_with_error

  validates :contributor_id, uniqueness: { allow_nil: true }

  def client_upload_access?
    client? && contributor&.project_upload_access?
  end
end
