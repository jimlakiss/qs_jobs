Rails.application.routes.draw do
  devise_for :users, skip: [:registrations]
  resource :labour_rates, only: [:show, :update]
  resources :labour_rate_averages, only: [:index]
  resources :labour_rate_sections, only: [:update]
  resources :labour_rate_categories, only: [:index, :create, :edit, :update, :destroy]
  get "hourly_rate/edit", to: redirect("/labour_rates")

  resources :project_statuses, only: [ :index, :create, :update, :destroy ]

  resources :projects do
    collection do
      get :current
    end
    resources :document_groups, only: [:update] do
      member do
        get :viewer
        get :download_working_documents
      end
    end

    resources :documents, only: [:create, :update, :destroy], controller: "project_documents" do
      member do
        get :viewer
        post :save_extraction
        get :viewer_state
        patch :viewer_state
        post :upload_export
      end
    end
    resources :document_extractions, only: [:destroy]
    resources :issues, except: [:index], controller: "project_issues" do
      member do
        delete "documents/:attachment_id", action: :destroy_document, as: :document
      end
    end

    member do
      get :confirm_destroy
      patch :schedule
      patch :archive
      patch :restore
    end
  end

  resources :client_submissions do
    resources :documents, only: [:create, :update, :destroy], controller: "client_submission_documents"

    member do
      post :submit
      post :convert
      post :attach_to_project
      post :archive
      post :unarchive
    end
  end

  get :client_area, to: "client_submissions#index"
  resources :issued_documents, only: [:index, :show]

  resources :contributors do
    resource :portal_access, only: [:create, :update, :destroy], controller: "contributor_portal_access"

    member do
      get :confirm_destroy
    end
  end

  resources :contributor_types do
    member do
      get :confirm_destroy
    end
  end

  root "projects#current"
end
