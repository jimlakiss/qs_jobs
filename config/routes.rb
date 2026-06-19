Rails.application.routes.draw do
  devise_for :users, skip: [:registrations]

  resources :projects do
    resources :documents, only: [:create, :update, :destroy], controller: "project_documents" do
      member do
        get :viewer
        post :save_extraction
        post :upload_export
      end
    end
    resources :document_extractions, only: [:destroy]

    member do
      get :confirm_destroy
    end
  end

  resources :contributors do
    member do
      get :confirm_destroy
    end
  end

  resources :contributor_types do
    member do
      get :confirm_destroy
    end
  end

  root "projects#index"
end
